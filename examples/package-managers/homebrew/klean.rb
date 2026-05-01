class Klean < Formula
  desc "Safe, efficient CLI for cleaning development environments"
  homepage "https://github.com/danil0ws/klean"
  url "https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-x86_64-apple-darwin.tar.gz"
  sha256 "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
  license "MIT OR Apache-2.0"

  bottle do
    sha256 cellar: :any_skip_relocation, arm64_sonoma:  "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
    sha256 cellar: :any_skip_relocation, x86_64_sonoma: "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
  end

  depends_on :macos

  def install
    bin.install "klean"
  end

  def post_install
    # Create initial config directory if needed
    config_dir = "#{Dir.home}/.config/klean"
    FileUtils.mkdir_p(config_dir) unless File.exist?(config_dir)
  end

  test do
    assert_match "A safe, efficient CLI", shell_output("#{bin}/klean --help")
    assert_match version.to_s, shell_output("#{bin}/klean --version")
  end
end
