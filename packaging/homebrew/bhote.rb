# Homebrew formula. Put it into a tap (a repo called homebrew-bhote, folder Formula/), then: brew install <user>/bhote/bhote
class Bhote < Formula
  desc "Topics, agents, servers and headless agents: a native add-on for the herdr Mac app, with the CLI"
  homepage "https://github.com/floriankappert/bhote"
  url "https://github.com/floriankappert/bhote.git", tag: "v0.9.3"
  license "MIT"
  head "https://github.com/floriankappert/bhote.git", branch: "main"

  depends_on "jq"

  def install
    bin.install "bhote"
    (pkgshare/"herdr-app").install Dir["herdr-app/*"]
    (pkgshare/"herdr-plugin").install Dir["herdr-plugin/*"]
    (pkgshare/"integrations/claude").install "integrations/claude/skills", "integrations/claude/mods", "integrations/claude/.claude-plugin"
    (pkgshare/"omarchy-plugin").install Dir["omarchy-plugin/*"]
    doc.install Dir["docs/*"]
  end

  def caveats
    <<~EOS
      Install the add-on into the herdr Mac app (topics, agents, servers, headless agents):
        bhote app install

      The terminal UI (deprecated since 0.9.3) opens its panel next to your agents when herdr starts:
        herdr plugin link #{opt_pkgshare}/herdr-plugin

      Let Claude Code do the rest of the setup (plugins, hooks, machines, monitors):
        bhote skills install
        then, in Claude Code: /bhote-install
    EOS
  end

  test do
    assert_match "bhote", shell_output("#{bin}/bhote help")
  end
end
