class Unstave < Formula
  desc "Module graph analyzer and barrel codemod for TypeScript monorepos"
  homepage "https://github.com/eddiesr93/unstave"
  url "https://github.com/eddiesr93/unstave.git", tag: "v0.1.0"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/unstave-cli")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/unstave --version")
  end
end
