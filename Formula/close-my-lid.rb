class CloseMyLid < Formula
  desc "Menu bar app that keeps a Mac awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"
  url "https://github.com/krishkalaria12/close-my-lid/archive/refs/tags/v0.5.0.tar.gz"
  version "0.5.0"
  sha256 "a339e08cd05c465ab954c6f864f8d12693b21a9521a1ba26b5f8da828f549c33"
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
