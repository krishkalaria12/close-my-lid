# frozen_string_literal: true

TAP_COMMAND = "brew tap krishkalaria12/close-my-lid https://github.com/krishkalaria12/close-my-lid"
APP_INSTALL_COMMAND = "brew install --cask krishkalaria12/close-my-lid/close-my-lid"

def shell_blocks(markdown)
  markdown.gsub("\r\n", "\n").scan(/```(?:sh|bash)\n(.*?)```/m).flatten
end

def complete_app_install_block?(markdown)
  shell_blocks(markdown).any? do |block|
    commands = block.lines.map(&:strip).reject(&:empty?)
    commands.each_cons(2).any? { |pair| pair == [TAP_COMMAND, APP_INSTALL_COMMAND] }
  end
end

documents = {
  "README.md" => File.read("README.md"),
  "docs/releasing.md" => File.read("docs/releasing.md")
}
documents["published release notes"] = ENV.fetch("RELEASE_BODY") if ENV.key?("RELEASE_BODY")

missing_documents = documents.reject { |_name, markdown| complete_app_install_block?(markdown) }.keys

unless missing_documents.empty?
  warn "Missing an ordered Homebrew app install block in: #{missing_documents.join(', ')}"
  exit 1
end

puts "Homebrew release instructions are complete"
