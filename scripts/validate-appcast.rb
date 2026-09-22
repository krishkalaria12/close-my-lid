#!/usr/bin/env ruby

# Checks that appcast.xml says what the app reads out of it.
#
# The app no longer downloads or installs from this feed — it reads the newest
# item's short version string, compares it against its own, and offers to open
# the release page. So the checks here are about the two things that can
# actually break a user: an item that the parser cannot read, and a download
# URL that is not an immutable, notarized GitHub Release archive.
#
# `sparkle:edSignature` is validated only for shape when present. It is kept on
# historical items but no longer required: nothing verifies it, because nothing
# is fetched and executed from this feed any more. Gatekeeper checks the
# signature and notarization of the archive the user actually downloads.
#
# The URL rule below is enforced a second time at runtime, by
# `lidcore::updates::is_release_url`: the app refuses to open an enclosure that
# is not under the project's own releases. This check is what stops a bad URL
# being committed; that one is what stops a tampered feed reaching a browser.

require "rexml/document"
require "uri"

path = ARGV.fetch(0, "appcast.xml")
document = REXML::Document.new(File.read(path))
errors = []
items = 0
short_versions = []

REXML::XPath.each(document, "/rss/channel/item") do |item|
  items += 1
  enclosure = item.elements["enclosure"]
  unless enclosure
    errors << "update item is missing an enclosure"
    next
  end

  version = enclosure.attributes["sparkle:version"] || item.elements["sparkle:version"]&.text
  short_version = enclosure.attributes["sparkle:shortVersionString"] ||
    item.elements["sparkle:shortVersionString"]&.text
  signature = enclosure.attributes["sparkle:edSignature"]
  url = enclosure.attributes["url"]
  length = enclosure.attributes["length"]

  errors << "update enclosure has a non-numeric sparkle:version" unless version&.match?(/\A\d+\z/)
  # This is the field the app compares against its own version. Without it an
  # update is published that no running app can ever notice.
  # Anchored at both ends: an unanchored match let `1.2.3-oops` through, and
  # the app compares this field as a semantic version.
  if short_version&.match?(/\A\d+(\.\d+)*\z/)
    short_versions << short_version
  else
    errors << "update item is missing a dotted sparkle:shortVersionString"
  end
  errors << "update enclosure has an invalid length" unless length&.match?(/\A[1-9]\d*\z/)
  unless signature.nil? || signature.match?(%r{\A[A-Za-z0-9+/]+={0,2}\z})
    errors << "update enclosure has a malformed sparkle:edSignature"
  end

  begin
    uri = URI.parse(url.to_s)
    valid_url = uri.is_a?(URI::HTTPS) &&
      uri.host == "github.com" &&
      uri.path.match?(%r{\A/krishkalaria12/close-my-lid/releases/download/v[^/]+/[^/]+\.zip\z})
    errors << "update enclosure must use an immutable GitHub Release ZIP URL" unless valid_url
  rescue URI::InvalidURIError
    errors << "update enclosure has an invalid URL"
  end
end

# An empty feed is silently treated as "no update available" forever.
errors << "appcast has no update items" if items.zero?

# The app picks the highest version rather than the first element, so a feed in
# the wrong order is no longer a broken update. It is still a broken *changelog*
# — this file is read by people too — and the drift is worth catching at the
# commit rather than at a release.
ordered = short_versions.sort_by { |version| version.split(".").map(&:to_i) }.reverse
if short_versions != ordered
  errors << "appcast items must run newest-first; expected #{ordered.join(', ')}"
end

abort(errors.join("\n")) unless errors.empty?
puts "Appcast is valid"
