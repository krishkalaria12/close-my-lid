cask "close-my-lid" do
  version "0.4.3"
  sha256 "ce81b37f65c1a5fae0939af20e6ea47d84c2601f054adb2ec42d616bf5bacdfa"

  url "https://github.com/krishkalaria12/close-my-lid/releases/download/v#{version}/Close-My-Lid-v#{version}-macOS.zip"
  name "Close My Lid"
  desc "Menu bar utility that keeps the computer awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"

  depends_on macos: :sonoma

  app "Close My Lid.app"

  zap trash: "~/Library/Preferences/app.closemylid.CloseMyLid.plist"
end
