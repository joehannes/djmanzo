import { describe, expect, it } from "vitest";
import { markdown } from "./markdown";

describe("blog markdown", () => {
  it("renders headings, lists, emphasis and links", () => {
    const html = markdown("## Mixing\n\nA **bold** and *soft* move.\n\n- one\n- two\n\n[docs](https://example.com)");
    expect(html).toContain("<h3>Mixing</h3>");
    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("<em>soft</em>");
    expect(html).toContain("<ul>\n<li>one</li>");
    expect(html).toContain('<a href="https://example.com"');
  });

  it("never lets markup through, whoever wrote the post", () => {
    const html = markdown('<script>alert(1)</script>\n\n[x](javascript:alert(1)) <img src=x onerror="y">');
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<img");
    expect(html).not.toContain('href="javascript');
  });
});
