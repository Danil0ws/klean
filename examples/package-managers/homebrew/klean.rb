class Klean < Formula
  desc "Safe, efficient CLI for cleaning development environments"
  homepage "https://github.com/danil0ws/klean"
  # Release asset names embed the target triple, so set the version explicitly
  # instead of letting Homebrew guess it from the URL.
  version "1.1.0"
  license "MIT OR Apache-2.0"

  # A tap formula only downloads the prebuilt binary: no Rust toolchain needed.
  # (Both sha256 values are filled in by .github/workflows/homebrew-update.yml.)
  if Hardware::CPU.arm?
    url "https://github.com/danil0ws/klean/releases/download/v1.1.0/klean-v1.1.0-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  else
    url "https://github.com/danil0ws/klean/releases/download/v1.1.0/klean-v1.1.0-x86_64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  depends_on :macos

  def install
    bin.install "klean"
  end

  test do
    assert_match "klean", shell_output("#{bin}/klean --version")
  end
end
