class CloseMyLid < Formula
  desc "Menu bar app that keeps a Mac awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"
  url "https://github.com/krishkalaria12/close-my-lid/archive/refs/tags/v0.3.0.tar.gz"
  version "0.3.0"
  sha256 "9f33eb088fef9bf7d729e6c9c98eb5f8b8081240838f56df01a0c2826c67afb4"
  license "MIT"

  depends_on macos: :sonoma

  def install
    system "swift", "build",
      "--package-path", "apps/macos",
      "--configuration", "release",
      "--product", "CloseMyLid",
      "--disable-sandbox"
    bin.install "apps/macos/.build/release/CloseMyLid" => "close-my-lid"
    prefix.install "apps/macos/.build/release/Sparkle.framework" => "Frameworks/Sparkle.framework"
  end

  test do
    assert_match "Close My Lid #{version}", shell_output("#{bin}/close-my-lid --version")
  end
end
