cask "close-my-lid" do
  version "0.5.1"
  sha256 "9876e9ae276611067c3c83223c40177a7ad2af6bdbac900b0973a3604fe5a749"

  url "https://github.com/krishkalaria12/close-my-lid/releases/download/v#{version}/Close-My-Lid-v#{version}-macOS.zip"
  name "Close My Lid"
  desc "Menu bar utility that keeps the computer awake with the lid closed"
  homepage "https://github.com/krishkalaria12/close-my-lid"

  depends_on macos: :sonoma

  app "Close My Lid.app"

  # The watchdog LaunchAgent has to go first: `launchctl` refuses to unload a
  # job whose plist has already been deleted, and left loaded it wakes every 60
  # seconds forever, pointing at a binary that is no longer there.
  uninstall launchctl: "app.closemylid.watchdog",
            delete:    "/etc/sudoers.d/close-my-lid"

  zap trash: [
    # The hold's session file, heartbeat and last-picked duration. `lidcore`
    # derives this from ProjectDirs("com", "krishkalaria", "close-my-lid"),
    # which is deliberately not the bundle identifier — see `lidcore::config`.
    "~/Library/Application Support/com.krishkalaria.close-my-lid",
    "~/Library/LaunchAgents/app.closemylid.watchdog.plist",
    "~/Library/Preferences/app.closemylid.CloseMyLid.plist",
  ]
end
