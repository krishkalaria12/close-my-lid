class CloseMyLid < Formula
  desc "Menu bar app that keeps a Mac awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"
  url "https://github.com/krishkalaria12/close-my-lid/archive/refs/tags/v0.6.1.tar.gz"
  version "0.6.1"
  sha256 "bc32f76f31954053ad9254a651138c58729b1269fdc026bd6b4e63dcf335bebc"
  license "MIT"

  depends_on "rust" => :build
  depends_on macos: :sonoma

  def install
    system "cargo", "install", *std_cargo_args(path: "apps/desktop/crates/lid-macos")
    # The bundle's executable is named for the app; the command is not.
    bin.install_symlink bin/"CloseMyLid" => "close-my-lid"
  end

  test do
    assert_match "Close My Lid #{version}", shell_output("#{bin}/close-my-lid --version")
  end
end
