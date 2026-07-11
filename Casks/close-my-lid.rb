cask "close-my-lid" do
  version "0.3.0"
  sha256 "49302b641b5bdfad85efdf733725973ec4e009d845ef04f2389a385a1c3c7b8d"

  url "https://github.com/krishkalaria12/close-my-lid/releases/download/v#{version}/Close-My-Lid-v#{version}-macOS.zip"
  name "Close My Lid"
  desc "Menu bar utility that keeps the computer awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"

  depends_on macos: :sonoma

  app "Close My Lid.app"

  zap trash: "~/Library/Preferences/app.closemylid.CloseMyLid.plist"
end
