#!/usr/bin/env ruby

require "rexml/document"
require "uri"

path = ARGV.fetch(0, "appcast.xml")
document = REXML::Document.new(File.read(path))
errors = []

REXML::XPath.each(document, "/rss/channel/item") do |item|
  enclosure = item.elements["enclosure"]
  unless enclosure
    errors << "update item is missing an enclosure"
    next
  end

  version = enclosure.attributes["sparkle:version"] || item.elements["sparkle:version"]&.text
  signature = enclosure.attributes["sparkle:edSignature"]
  url = enclosure.attributes["url"]
  length = enclosure.attributes["length"]

  errors << "update enclosure has a non-numeric sparkle:version" unless version&.match?(/\A\d+\z/)
  errors << "update enclosure is missing sparkle:edSignature" if signature.to_s.empty?
  errors << "update enclosure has an invalid length" unless length&.match?(/\A[1-9]\d*\z/)

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

abort(errors.join("\n")) unless errors.empty?
puts "Appcast is valid"
