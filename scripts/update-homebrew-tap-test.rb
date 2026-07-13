# frozen_string_literal: true

require "digest"
require "fileutils"
require "minitest/autorun"
require "open3"
require "rbconfig"
require "tmpdir"

class UpdateHomebrewTapTest < Minitest::Test
  SCRIPT = File.expand_path("update-homebrew-tap.rb", __dir__)
  TAP_FIXTURE = File.expand_path("fixtures/homebrew-tap", __dir__)
  ARTIFACT_FIXTURE = File.expand_path("fixtures/artifacts", __dir__)
  SOURCE_ARCHIVE = File.join(ARTIFACT_FIXTURE, "close-my-lid-v1.2.3.tar.gz")
  APP_ARCHIVE = File.join(ARTIFACT_FIXTURE, "Close-My-Lid-v1.2.3-macOS.zip")
  SOURCE_URL = "https://github.com/example/close-my-lid/archive/refs/tags/v1.2.3.tar.gz"
  APP_URL = "https://github.com/example/close-my-lid/releases/download/v1.2.3/Close-My-Lid-v1.2.3-macOS.zip"

  def setup
    @temporary_directory = Dir.mktmpdir("homebrew-tap-test")
    @tap_directory = File.join(@temporary_directory, "tap")
    FileUtils.cp_r(TAP_FIXTURE, @tap_directory)
  end

  def teardown
    FileUtils.remove_entry(@temporary_directory)
  end

  def run_updater(*arguments)
    Open3.capture3(
      RbConfig.ruby,
      SCRIPT,
      "--tap-dir", @tap_directory,
      *arguments,
    )
  end

  def archive_arguments
    [
      "--tag", "v1.2.3",
      "--source-url", SOURCE_URL,
      "--app-url", APP_URL,
      "--source-archive", SOURCE_ARCHIVE,
      "--app-archive", APP_ARCHIVE,
    ]
  end

  def test_updates_both_definitions_with_computed_fixture_checksums
    stdout, stderr, status = run_updater(*archive_arguments)

    assert status.success?, stderr
    assert_equal "changed\n", stdout

    formula = File.read(File.join(@tap_directory, "Formula/close-my-lid.rb"))
    cask = File.read(File.join(@tap_directory, "Casks/close-my-lid.rb"))
    assert_includes formula, %(url "#{SOURCE_URL}")
    assert_includes formula, %(sha256 "#{Digest::SHA256.file(SOURCE_ARCHIVE).hexdigest}")
    assert_includes cask, %(version "1.2.3")
    assert_includes cask, %(sha256 "#{Digest::SHA256.file(APP_ARCHIVE).hexdigest}")
    assert_includes cask, 'url "https://github.com/example/close-my-lid/releases/download/v#{version}/Close-My-Lid-v#{version}-macOS.zip"'
    refute_match(/^\s*version\s+/, formula)
  end

  def test_second_update_is_a_no_op
    _stdout, stderr, status = run_updater(*archive_arguments)
    assert status.success?, stderr

    stdout, stderr, status = run_updater(*archive_arguments)

    assert status.success?, stderr
    assert_equal "no-change\n", stdout
  end

  def test_builds_release_urls_from_repository_metadata_and_accepts_checksums
    source_sha256 = "1" * 64
    app_sha256 = "2" * 64

    stdout, stderr, status = run_updater(
      "--tag", "v1.2.3",
      "--repository", "example/close-my-lid",
      "--source-sha256", source_sha256,
      "--app-sha256", app_sha256,
    )

    assert status.success?, stderr
    assert_equal "changed\n", stdout
    assert_includes File.read(File.join(@tap_directory, "Formula/close-my-lid.rb")), %(sha256 "#{source_sha256}")
    assert_includes File.read(File.join(@tap_directory, "Casks/close-my-lid.rb")), %(sha256 "#{app_sha256}")
  end

  def test_rejects_a_non_semantic_version_tag_without_modifying_the_tap
    before = tap_contents

    _stdout, stderr, status = run_updater(*archive_arguments.map { |value| value == "v1.2.3" ? "version-1" : value })

    refute status.success?
    assert_includes stderr, "tag must be a semantic version"
    assert_equal before, tap_contents
  end

  def test_rejects_a_prerelease_by_default
    _stdout, stderr, status = run_updater(
      "--tag", "v1.2.3-beta.1",
      "--repository", "example/close-my-lid",
      "--source-sha256", "1" * 64,
      "--app-sha256", "2" * 64,
    )

    refute status.success?
    assert_includes stderr, "prerelease tags require --allow-prerelease"
  end

  def test_rejects_a_downgrade_without_modifying_the_tap
    before = tap_contents

    _stdout, stderr, status = run_updater(
      "--tag", "v0.2.0",
      "--repository", "example/close-my-lid",
      "--source-sha256", "1" * 64,
      "--app-sha256", "2" * 64,
    )

    refute status.success?
    assert_includes stderr, "refusing to downgrade tap from 0.3.0 to 0.2.0"
    assert_equal before, tap_contents
  end

  def test_rejects_changed_metadata_for_the_current_version
    before = tap_contents

    _stdout, stderr, status = run_updater(
      "--tag", "v0.3.0",
      "--repository", "example/close-my-lid",
      "--source-sha256", "1" * 64,
      "--app-sha256", "2" * 64,
    )

    refute status.success?
    assert_includes stderr, "already contains version 0.3.0 with different release metadata"
    assert_equal before, tap_contents
  end

  def test_rejects_an_app_asset_that_does_not_match_the_tag
    arguments = archive_arguments.dup
    arguments[arguments.index(APP_URL)] = "https://github.com/example/close-my-lid/releases/download/v1.2.3/wrong.zip"

    _stdout, stderr, status = run_updater(*arguments)

    refute status.success?
    assert_includes stderr, "app URL must end with Close-My-Lid-v1.2.3-macOS.zip"
  end

  def test_rejects_a_checksum_that_does_not_match_the_supplied_archive
    _stdout, stderr, status = run_updater(
      *archive_arguments,
      "--source-sha256", "f" * 64,
    )

    refute status.success?
    assert_includes stderr, "source checksum does not match"
  end

  def test_rejects_missing_required_tap_file_without_partial_update
    FileUtils.rm(File.join(@tap_directory, "Casks/close-my-lid.rb"))
    formula_before = File.read(File.join(@tap_directory, "Formula/close-my-lid.rb"))

    _stdout, stderr, status = run_updater(*archive_arguments)

    refute status.success?
    assert_includes stderr, "missing tap file"
    assert_equal formula_before, File.read(File.join(@tap_directory, "Formula/close-my-lid.rb"))
  end

  def test_rejects_a_missing_checksum_pattern_without_partial_update
    cask_path = File.join(@tap_directory, "Casks/close-my-lid.rb")
    File.write(cask_path, File.read(cask_path).sub(/^\s*sha256.*\n/, ""))
    before = tap_contents

    _stdout, stderr, status = run_updater(*archive_arguments)

    refute status.success?
    assert_includes stderr, "could not find cask checksum"
    assert_equal before, tap_contents
  end

  def test_rejects_missing_url_metadata
    _stdout, stderr, status = run_updater(
      "--tag", "v1.2.3",
      "--source-sha256", "1" * 64,
      "--app-sha256", "2" * 64,
    )

    refute status.success?
    assert_includes stderr, "provide --repository or both --source-url and --app-url"
  end

  private

  def tap_contents
    Dir.glob(File.join(@tap_directory, "**/*")).select { |path| File.file?(path) }.sort.to_h do |path|
      [path.delete_prefix("#{@tap_directory}/"), File.binread(path)]
    end
  end
end
