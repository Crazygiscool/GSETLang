class Gset < Formula
  desc "Generic Syntax Extension Tool - write in any language syntax, compile to any language"
  homepage "https://github.com/Crazygiscool/GSETLang"
  url "https://github.com/Crazygiscool/GSETLang/archive/refs/tags/v3.2.1.tar.gz"
  sha256 "FILL_AFTER_RELEASE"
  license "MIT OR Apache-2.0"
  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--locked", "-p", "gset-cli"
    bin.install "target/release/gset"
  end

  test do
    system "#{bin}/gset", "--version"
  end
end
