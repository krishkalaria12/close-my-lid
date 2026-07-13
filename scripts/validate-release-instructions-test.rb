# frozen_string_literal: true

require "minitest/autorun"
require "open3"
require "rbconfig"

class ValidateReleaseInstructionsTest < Minitest::Test
  TAP_COMMAND = "brew tap krishkalaria12/close-my-lid https://github.com/krishkalaria12/close-my-lid"
  APP_INSTALL_COMMAND = "brew install --cask krishkalaria12/close-my-lid/close-my-lid"
  VALIDATOR = File.expand_path("validate-release-instructions.rb", __dir__)

  def validate(release_body)
    Open3.capture3({ "RELEASE_BODY" => release_body }, RbConfig.ruby, VALIDATOR)
  end

  def test_accepts_tap_immediately_before_install
    _stdout, stderr, status = validate(<<~MARKDOWN)
      ```sh
      #{TAP_COMMAND}
      #{APP_INSTALL_COMMAND}
      ```
    MARKDOWN

    assert status.success?, stderr
  end

  def test_accepts_crlf_release_notes
    body = "```sh\r\n#{TAP_COMMAND}\r\n#{APP_INSTALL_COMMAND}\r\n```\r\n"
    _stdout, stderr, status = validate(body)

    assert status.success?, stderr
  end

  def test_rejects_install_before_tap
    _stdout, _stderr, status = validate(<<~MARKDOWN)
      ```sh
      #{APP_INSTALL_COMMAND}
      #{TAP_COMMAND}
      ```
    MARKDOWN

    refute status.success?
  end

  def test_rejects_release_body_without_fresh_install_instructions
    _stdout, _stderr, status = validate("## What changed\n\nBug fixes.\n")

    refute status.success?
  end
end
