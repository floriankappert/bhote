# Homebrew formula. Put it into a tap (a repo called homebrew-bhote, folder Formula/), then: brew install <user>/bhote/bhote
class Bhote < Formula
  desc "Topics and agents side panel for the herdr terminal workspace manager"
  homepage "https://github.com/floriankappert/bhote"
  url "https://github.com/floriankappert/bhote.git", tag: "v0.1.0"
  license "MIT"
  head "https://github.com/floriankappert/bhote.git", branch: "main"

  depends_on "jq"

  def install
    bin.install "bhote"
    (pkgshare/"herdr-plugin").install Dir["herdr-plugin/*"]
  end

  def caveats
    <<~EOS
      To open the panel automatically next to your agents when herdr starts:
        herdr plugin link #{opt_pkgshare}/herdr-plugin
    EOS
  end

  test do
    assert_match "bhote", shell_output("#{bin}/bhote help")
  end
end
