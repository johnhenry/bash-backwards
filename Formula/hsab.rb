class Hsab < Formula
  desc "Stack-based postfix shell with persistent state between commands"
  homepage "https://github.com/johnhenry/bash-backwards"
  url "https://github.com/johnhenry/bash-backwards.git",
      tag:      "v0.1.0",
      revision: ""
  license "MIT"
  head "https://github.com/johnhenry/bash-backwards.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  def post_install
    ohai "Run 'hsab init' to install the standard library to ~/.hsab/lib/"
  end

  test do
    assert_match "hsab-0.1.0", shell_output("#{bin}/hsab --version")
    assert_equal "hello", shell_output("#{bin}/hsab -c '\"hello\" echo'").strip
  end
end
