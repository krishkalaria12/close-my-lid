# frozen_string_literal: true

APP_INSTALL_COMMAND = "brew install --cask krishkalaria12/close-my-lid/close-my-lid"
UNTAP_COMMAND = "brew untap --force krishkalaria12/close-my-lid"
TAP_COMMAND = "brew tap krishkalaria12/close-my-lid"

def shell_blocks(markdown)
  markdown.gsub("\r\n", "\n").scan(/```(?:sh|bash)\n(.*?)```/m).flatten
end

def shell_commands(block)
  block.gsub(/\\\s*\n\s*/, " ").lines.map { |line| line.split.join(" ") }.reject(&:empty?)
end

def standalone_app_install_block?(markdown)
  shell_blocks(markdown).any? do |block|
    shell_commands(block) == [APP_INSTALL_COMMAND]
  end
end

def migration_block?(markdown)
  shell_blocks(markdown).any? do |block|
    shell_commands(block).each_cons(2).any? { |pair| pair == [UNTAP_COMMAND, TAP_COMMAND] }
  end
end

documents = {
  "README.md" => File.read("README.md"),
  "docs/releasing.md" => File.read("docs/releasing.md")
}
documents["published release notes"] = ENV.fetch("RELEASE_BODY") if ENV.key?("RELEASE_BODY")

missing_install = documents.reject { |_name, markdown| standalone_app_install_block?(markdown) }.keys
missing_migration = documents.reject { |_name, markdown| migration_block?(markdown) }.keys

unless missing_install.empty? && missing_migration.empty?
  warn "Missing a standalone Homebrew app install block in: #{missing_install.join(', ')}" unless missing_install.empty?
  warn "Missing Homebrew tap migration instructions in: #{missing_migration.join(', ')}" unless missing_migration.empty?
  exit 1
end

puts "Homebrew release instructions are complete"
