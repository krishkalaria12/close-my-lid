# frozen_string_literal: true

require "minitest/autorun"
require "open3"
require "rbconfig"

class ValidateReleaseInstructionsTest < Minitest::Test
  APP_INSTALL_COMMAND = "brew install --cask krishkalaria12/close-my-lid/close-my-lid"
  UNTAP_COMMAND = "brew untap --force krishkalaria12/close-my-lid"
  TAP_COMMAND = "brew tap krishkalaria12/close-my-lid"
  VALIDATOR = File.expand_path("validate-release-instructions.rb", __dir__)

  def validate(release_body)
    Open3.capture3({ "RELEASE_BODY" => release_body }, RbConfig.ruby, VALIDATOR)
  end

  def complete_release_body(line_ending: "\n")
    [
      "```sh",
      APP_INSTALL_COMMAND,
      "```",
      "",
      "```sh",
      UNTAP_COMMAND,
      TAP_COMMAND,
      "```",
      ""
    ].join(line_ending)
  end

  def test_accepts_standalone_install_and_migration_commands
    _stdout, stderr, status = validate(complete_release_body)

    assert status.success?, stderr
  end

  def test_accepts_crlf_release_notes
    _stdout, stderr, status = validate(complete_release_body(line_ending: "\r\n"))

    assert status.success?, stderr
  end

  def test_rejects_old_custom_tap_install_block
    _stdout, stderr, status = validate(<<~MARKDOWN)
      ```sh
      brew tap krishkalaria12/close-my-lid https://github.com/krishkalaria12/close-my-lid
      #{APP_INSTALL_COMMAND}
      ```

      ```sh
      #{UNTAP_COMMAND}
      #{TAP_COMMAND}
      ```
    MARKDOWN

    refute status.success?
  end

  def test_rejects_release_body_without_migration_instructions
    _stdout, _stderr, status = validate(<<~MARKDOWN)
      ```sh
      #{APP_INSTALL_COMMAND}
      ```
    MARKDOWN

    refute status.success?
  end

  def test_rejects_release_body_without_fresh_install_instructions
    _stdout, _stderr, status = validate("## What changed\n\nBug fixes.\n")

    refute status.success?
  end
end
