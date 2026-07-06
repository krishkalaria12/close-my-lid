cask "close-my-lid" do
  version "0.1.0"
  sha256 "58293ab03a70b05aca3098ad681eef835c0c16dd0bca7be1ded3774b74a8cb0c"

  url "https://github.com/krishkalaria12/close-my-lid/releases/download/v#{version}/Close-My-Lid-v#{version}-macOS.zip"
  name "Close My Lid"
  desc "Menu bar utility that keeps the computer awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"

  depends_on macos: :sonoma

  app "Close My Lid.app"

  zap trash: "~/Library/Preferences/app.closemylid.CloseMyLid.plist"
end
