class Gset < Formula
  desc "Transpile any language syntax to any target language"
  homepage "https://github.com/Crazygiscool/GSETLang"
  url "https://github.com/Crazygiscool/GSETLang/archive/refs/tags/v3.2.1.tar.gz"
  sha256 "FILL_AFTER_RELEASE"
  license any_of: ["MIT", "Apache-2.0"]
  head "https://github.com/Crazygiscool/GSETLang.git", branch: "main"

  livecheck do
    url :stable
    strategy :github_latest
  end

  depends_on "rust" => :build

  def fetch
    system "cargo", "fetch", *std_cargo_fetch_args
  end

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/gset-cli")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gset --version")
    assert_match "GSET v#{version}", shell_output("#{bin}/gset version")
  end
end
