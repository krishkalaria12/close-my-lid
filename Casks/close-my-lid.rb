cask "close-my-lid" do
  version "0.4.4"
  sha256 "21e5eff381d4ad7bf6635450a178f19b73e60fc5cd5f40001cb4fcf741bb91f5"

  url "https://github.com/krishkalaria12/close-my-lid/releases/download/v#{version}/Close-My-Lid-v#{version}-macOS.zip"
  name "Close My Lid"
  desc "Menu bar utility that keeps the computer awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"

  depends_on macos: :sonoma

  app "Close My Lid.app"

  zap trash: "~/Library/Preferences/app.closemylid.CloseMyLid.plist"
end
