#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "open-uri"
require "optparse"
require "pathname"
require "rubygems/version"
require "tempfile"
require "timeout"
require "uri"

class HomebrewTapUpdateError < StandardError; end

class HomebrewTapUpdater
  SEMANTIC_TAG = /\Av(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\z/
  SHA256 = /\A[0-9a-f]{64}\z/i

  def initialize(options)
    @options = options
  end

  def run
    tag = required_option(:tag)
    match = SEMANTIC_TAG.match(tag)
    raise HomebrewTapUpdateError, "tag must be a semantic version beginning with v (for example, v1.2.3)" unless match

    version = tag.delete_prefix("v")
    if version.include?("-") && !@options[:allow_prerelease]
      raise HomebrewTapUpdateError, "prerelease tags require --allow-prerelease"
    end
    source_url, app_url = release_urls(tag, version)
    validate_release_urls(source_url, app_url, tag, version)

    tap_directory = Pathname(required_option(:tap_dir)).expand_path
    formula_path = tap_directory.join("Formula/close-my-lid.rb")
    cask_path = tap_directory.join("Casks/close-my-lid.rb")
    missing_paths = [formula_path, cask_path].reject(&:file?)
    unless missing_paths.empty?
      raise HomebrewTapUpdateError, "missing tap file: #{missing_paths.map(&:to_s).join(", ")}"
    end

    formula_before = formula_path.read
    cask_before = cask_path.read
    current_version = cask_version(cask_before)
    comparison = Gem::Version.new(version) <=> Gem::Version.new(current_version)
    if comparison.negative?
      raise HomebrewTapUpdateError, "refusing to downgrade tap from #{current_version} to #{version}"
    end

    source_sha256 = checksum_for("source", source_url, @options[:source_archive], @options[:source_sha256])
    app_sha256 = checksum_for("app", app_url, @options[:app_archive], @options[:app_sha256])

    formula_after = update_formula(formula_before, source_url, source_sha256, version)
    cask_after = update_cask(cask_before, app_url, app_sha256, version)

    if formula_before == formula_after && cask_before == cask_after
      puts "no-change"
      return
    end
    if comparison.zero?
      raise HomebrewTapUpdateError, "tap already contains version #{version} with different release metadata"
    end

    formula_path.write(formula_after)
    cask_path.write(cask_after)
    puts "changed"
  end

  private

  def required_option(name)
    value = @options[name]
    raise HomebrewTapUpdateError, "missing required option --#{name.to_s.tr("_", "-")}" if value.nil? || value.empty?

    value
  end

  def release_urls(tag, version)
    repository = @options[:repository]
    source_url = @options[:source_url]
    app_url = @options[:app_url]

    if repository
      unless repository.match?(/\A[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\z/)
        raise HomebrewTapUpdateError, "repository must have the form owner/name"
      end
      if source_url || app_url
        raise HomebrewTapUpdateError, "use --repository or explicit URLs, not both"
      end

      source_url = "https://github.com/#{repository}/archive/refs/tags/#{tag}.tar.gz"
      app_url = "https://github.com/#{repository}/releases/download/#{tag}/Close-My-Lid-v#{version}-macOS.zip"
    elsif source_url.nil? || app_url.nil?
      raise HomebrewTapUpdateError, "provide --repository or both --source-url and --app-url"
    end

    [source_url, app_url]
  end

  def validate_release_urls(source_url, app_url, tag, version)
    source_uri = parse_https_url("source", source_url)
    app_uri = parse_https_url("app", app_url)
    unless source_uri.path.end_with?("/archive/refs/tags/#{tag}.tar.gz")
      raise HomebrewTapUpdateError, "source URL must end with /archive/refs/tags/#{tag}.tar.gz"
    end

    expected_asset = "Close-My-Lid-v#{version}-macOS.zip"
    unless File.basename(app_uri.path) == expected_asset
      raise HomebrewTapUpdateError, "app URL must end with #{expected_asset}"
    end
    unless app_uri.path.include?("/releases/download/#{tag}/")
      raise HomebrewTapUpdateError, "app URL must reference release #{tag}"
    end
  end

  def parse_https_url(label, value)
    uri = URI.parse(value)
    unless uri.is_a?(URI::HTTPS) && uri.host && !uri.host.empty?
      raise HomebrewTapUpdateError, "#{label} URL must be an HTTPS URL"
    end

    uri
  rescue URI::InvalidURIError
    raise HomebrewTapUpdateError, "#{label} URL is malformed"
  end

  def checksum_for(label, url, archive, supplied_checksum)
    if supplied_checksum && !supplied_checksum.match?(SHA256)
      raise HomebrewTapUpdateError, "#{label} checksum must be 64 hexadecimal characters"
    end

    if archive
      path = Pathname(archive).expand_path
      raise HomebrewTapUpdateError, "#{label} archive does not exist: #{path}" unless path.file?

      computed_checksum = Digest::SHA256.file(path).hexdigest
      if supplied_checksum && supplied_checksum.downcase != computed_checksum
        raise HomebrewTapUpdateError, "#{label} checksum does not match supplied archive"
      end
      return computed_checksum
    end

    return supplied_checksum.downcase if supplied_checksum

    checksum_download(url, label)
  end

  def checksum_download(url, label)
    attempts = 0
    begin
      attempts += 1
      digest = Digest::SHA256.new
      URI.open(url, "rb", open_timeout: 15, read_timeout: 60) do |download|
        while (chunk = download.read(1024 * 1024))
          digest.update(chunk)
        end
      end
      digest.hexdigest
    rescue OpenURI::HTTPError, SocketError, SystemCallError, Timeout::Error => error
      retry if attempts < 3

      raise HomebrewTapUpdateError, "could not download #{label} asset after #{attempts} attempts: #{error.message}"
    end
  end

  def cask_version(contents)
    match = contents.match(/^\s*version\s+"([^"]+)"/)
    raise HomebrewTapUpdateError, "could not find cask version" unless match
    unless match[1].match?(/\A(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\z/)
      raise HomebrewTapUpdateError, "existing cask version is not semantic: #{match[1]}"
    end

    match[1]
  end

  def update_formula(contents, source_url, source_sha256, version)
    updated = replace_required(contents, /^([ \t]*)url\s+"[^"]+"/, %(\\1url "#{source_url}"), "formula URL")
    updated = replace_required(updated, /^([ \t]*)sha256\s+"[0-9a-fA-F]+"/, %(\\1sha256 "#{source_sha256}"), "formula checksum")
    updated.sub(/^([ \t]*)version\s+"[^"]+"/, %(\\1version "#{version}"))
  end

  def update_cask(contents, app_url, app_sha256, version)
    updated = replace_required(contents, /^([ \t]*)version\s+"[^"]+"/, %(\\1version "#{version}"), "cask version")
    updated = replace_required(updated, /^([ \t]*)sha256\s+"[0-9a-fA-F]+"/, %(\\1sha256 "#{app_sha256}"), "cask checksum")
    url_match = updated.match(/^([ \t]*)url\s+"([^"]+)"/)
    raise HomebrewTapUpdateError, "could not find cask URL" unless url_match

    templated_app_url = app_url.gsub("v#{version}", 'v#{version}')
    updated.sub(url_match[0], %(#{url_match[1]}url "#{templated_app_url}"))
  end

  def replace_required(contents, pattern, replacement, description)
    raise HomebrewTapUpdateError, "could not find #{description}" unless contents.match?(pattern)

    contents.sub(pattern, replacement)
  end
end

options = {}
parser = OptionParser.new do |arguments|
  arguments.banner = "Usage: update-homebrew-tap.rb --tap-dir PATH --tag vX.Y.Z (--repository OWNER/REPO | --source-url URL --app-url URL) [options]"
  arguments.on("--tap-dir PATH", "Homebrew tap checkout") { |value| options[:tap_dir] = value }
  arguments.on("--tag TAG", "Release tag, such as v1.2.3") { |value| options[:tag] = value }
  arguments.on("--repository OWNER/REPO", "Build GitHub release URLs from repository metadata") { |value| options[:repository] = value }
  arguments.on("--source-url URL", "Source archive URL") { |value| options[:source_url] = value }
  arguments.on("--app-url URL", "macOS application archive URL") { |value| options[:app_url] = value }
  arguments.on("--source-archive PATH", "Local source archive used to compute or verify SHA-256") { |value| options[:source_archive] = value }
  arguments.on("--app-archive PATH", "Local application archive used to compute or verify SHA-256") { |value| options[:app_archive] = value }
  arguments.on("--source-sha256 SHA256", "Precomputed source SHA-256") { |value| options[:source_sha256] = value }
  arguments.on("--app-sha256 SHA256", "Precomputed application SHA-256") { |value| options[:app_sha256] = value }
  arguments.on("--allow-prerelease", "Allow a prerelease semantic version") { options[:allow_prerelease] = true }
end

begin
  parser.parse!
  raise HomebrewTapUpdateError, "unexpected arguments: #{ARGV.join(" ")}" unless ARGV.empty?

  HomebrewTapUpdater.new(options).run
rescue OptionParser::ParseError, HomebrewTapUpdateError => error
  warn "error: #{error.message}"
  exit 1
end
