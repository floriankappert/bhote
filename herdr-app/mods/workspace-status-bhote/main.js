import { View, div, svg } from "gpui-kit";
import { h_flex } from "gpui-base";
import { spaces, agents, theme, run, window, keys, updates, layout } from "herdr";

export default class WorkspaceStatus extends View {
  // The palette key as actually bound, not a hardcoded one; without a
  // binding the hint stays useful ("commands") instead of lying.
  hint(t) {
    const mac = window()?.platform === "macos";
    const mod = { cmd: mac ? "⌘" : "Ctrl+", ctrl: mac ? "⌃" : "Ctrl+", shift: mac ? "⇧" : "Shift+", alt: mac ? "⌥" : "Alt+", option: mac ? "⌥" : "Alt+" };
    const b = keys().find((k) => k.section === "global" && k.action === "app.palette");
    const parts = b && b.keys.endsWith("--") ? [...b.keys.slice(0, -2).split("-"), "-"] : b?.keys.split("-");
    const key = parts && parts.map((p) => mod[p.toLowerCase()] ?? p.toUpperCase()).join("");
    return key ? `${key}  commands` : "commands";
  }

  action(t, id, icon, text, onClick) {
    return h_flex().id(id).gap(6).items_center().cursor_pointer().hover(s => s.text_color(t.foreground)).on_click(onClick)
      .child(svg(`mods/workspace-chrome/${icon}.svg`).size(13)).child(text);
  }

  // The width of the sidebar (its edge in the layout): what stands after Herdr's two actions begins where the work column's pane begins.
  sidebarPx() {
    const find = (n, id) => { if (n?.id === id) return n; for (const c of n?.children ?? []) { const f = find(c, id); if (f) return f; } return null; };
    return find(layout(), "sidebar")?.size?.px ?? 268;
  }

  render() {
    const t = theme();
    const space = spaces().find(s => s.active);
    const list = agents();
    const needs = list.filter(a => a.status === "blocked").length;
    const label = (icon, text) => h_flex().gap(6).items_center().child(svg(`mods/workspace-chrome/${icon}.svg`).size(13)).child(text);
    const left = this.sidebarPx() + 1 + 8;   // (sidebar, divider, the padding of the work body: the left edge of the pane)
    return h_flex().size_full().gap(16).items_center().bg(t.chrome).border_t(1).border_color(t.border).text_size(11.8).text_color(t.muted)
      // Herdr's own settings and add machine, at the left end, in the width of the sidebar
      .child(h_flex().w(left - 16).flex_none().pl(14).gap(16).items_center()
      .child(this.action(t, "settings", "settings", "Settings", () => run("app.settings"))
        .when(Boolean(updates().app.ready), el => el.child(div().size(6).rounded_full().bg(t.accent))))
      .child(this.action(t, "machine", "plus", "Add machine", () => run("mod.open", { name: "machine-add" }))))
      .child(label(space?.machine ? "server" : "monitor", space?.machine ?? "This computer"))
      .when(Boolean(space?.branch), el => el.child(label("branch", space.branch)))
      .child(h_flex().gap(6).items_center().child(div().size(6).rounded(3).bg(t.dim)).child(`${list.length} agents`)
        .when(needs > 0, el => el.child(div().text_color(t.blocked).child(`· ${needs} needs you`))))
      .child(div().flex_1())
      .child(div().id("palette").pr(14).text_color(t.dim).on_click(() => run("app.palette")).child(this.hint(t)));
  }
}
