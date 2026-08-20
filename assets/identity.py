#!/usr/bin/env python3
"""Renders the generic "what this policy is" figures.

Five stylistic takes on the same idea, for slides and the Exchange page:

    policy-hub.png       obligation clusters feeding one agent answer
    policy-facets.png    the five facets of the contract layer
    policy-layers.png    outputSchema vs contract vs injected guidance
    policy-board.png     upstream, contract layer, consumers, and the pushback
    policy-ontology.png  the domain model, as labelled relations

    python3 assets/identity.py
"""
import pathlib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.patches import Circle, FancyArrowPatch, FancyBboxPatch, Polygon

OUT_DIR = pathlib.Path(__file__).resolve().parent

BG = "#f6f8fb"
INK = "#0f172a"
BODY = "#334155"
MUTED = "#64748b"
RULE_LINE = "#d3dae4"
WHITE = "#ffffff"

BLUE, BLUE_FILL = "#3b5bdb", "#e7ecfd"
AMBER, AMBER_FILL = "#c9821a", "#fdf0d5"
PLUM, PLUM_FILL = "#6741d9", "#ece7fb"
GREEN, GREEN_FILL = "#3f9160", "#e8f3ec"
MONO = "DejaVu Sans Mono"


def canvas(w, h):
    fig, ax = plt.subplots(figsize=(w, h), dpi=110)
    fig.patch.set_facecolor(BG)
    ax.set_facecolor(BG)
    ax.set_xlim(0, w)
    ax.set_ylim(0, h)
    ax.axis("off")
    return fig, ax


def box(ax, x, y, w, h, fill, edge, lw=1.5, z=2, rounding=0.16):
    ax.add_patch(
        FancyBboxPatch(
            (x, y), w, h,
            boxstyle=f"round,pad=0.05,rounding_size={rounding}",
            facecolor=fill, edgecolor=edge, linewidth=lw, zorder=z,
        )
    )


def heading(ax, w, h, title, subtitle=""):
    ax.text(0.4, h - 0.5, title, fontsize=19, fontweight="bold", color=INK, va="center")
    if subtitle:
        ax.text(0.4, h - 1.05, subtitle, fontsize=11, color=MUTED, va="center")


def footer(ax, w, note):
    ax.plot([0.4, w - 0.4], [0.62, 0.62], color=RULE_LINE, linewidth=1)
    ax.text(0.4, 0.28, note, fontsize=9.5, color=MUTED, va="center")


def save(fig, name):
    path = OUT_DIR / name
    fig.savefig(path, facecolor=BG, bbox_inches="tight", pad_inches=0.3)
    plt.close(fig)
    print("wrote", path.name)


# ---------------------------------------------------------------------------
# Style 1 — hub model: rules cluster into the obligation they impose.
# ---------------------------------------------------------------------------

CLUSTERS = [
    (
        "Do not promise", AMBER, AMBER_FILL,
        [
            ("batch-under-\nrecall", "Quality Assurance", "QN-2026-0412"),
            ("carrier-service-\nsuspended", "Logistics", "LOG-INC-2026-0231"),
        ],
    ),
    (
        "Do not disclose", PLUM, PLUM_FILL,
        [
            ("export-licence-\nmissing", "Trade Compliance", "TC-114"),
            ("customer-\ncommunication-\nhold", "Legal", "LEG-2026-0088"),
        ],
    ),
    (
        "Do not quote", GREEN, GREEN_FILL,
        [
            ("material-\nsuperseded", "Engineering", "ECN-4471"),
            ("legacy-plant-\npricing", "Finance", "FI-MIG-1042"),
        ],
    ),
]


def render_hub():
    w, h = 17.0, 11.2
    fig, ax = canvas(w, h)
    heading(
        ax, w, h,
        "MCP Semantic Contract",
        "Six facts, six owners, one answer — reconciled at the gateway, per response",
    )

    agent_w, agent_h = 4.8, 0.95
    agent_x = w / 2 - agent_w / 2
    agent_y = 8.6
    box(ax, agent_x, agent_y, agent_w, agent_h, BLUE_FILL, BLUE, lw=1.8)
    ax.text(w / 2, agent_y + agent_h / 2, "What the agent may say",
            fontsize=13.5, fontweight="bold", color=BLUE, ha="center", va="center")

    bus_y = 7.9
    centres = [3.2, 8.5, 13.8]
    pillar_y, pillar_w, pillar_h = 6.3, 2.5, 1.15
    circle_y, radius = 4.4, 0.8

    ax.plot([centres[0], centres[-1]], [bus_y, bus_y], color=BLUE, linewidth=1.5, zorder=1)
    ax.add_patch(FancyArrowPatch(
        (w / 2, bus_y), (w / 2, agent_y - 0.04),
        arrowstyle="-|>,head_length=7,head_width=4",
        color=BLUE, linewidth=1.8, shrinkA=0, shrinkB=0, zorder=3))

    for cx, (label, edge, fill, rules) in zip(centres, CLUSTERS):
        ax.plot([cx, cx], [pillar_y + pillar_h, bus_y], color=BLUE, linewidth=1.5, zorder=1)

        for sign, (rule, owner, ref) in zip((-1, 1), rules):
            rx = cx + sign * 1.3
            ax.add_patch(Circle((rx, circle_y), radius, facecolor=fill, edgecolor=edge,
                                linewidth=1.5, zorder=2))
            ax.text(rx, circle_y, rule, fontsize=7.0, color=INK, ha="center",
                    va="center", family=MONO, zorder=3, linespacing=1.3)
            ax.text(rx, circle_y - 1.18, owner, fontsize=8.8, color=BODY,
                    ha="center", va="center", fontweight="bold")
            ax.text(rx, circle_y - 1.55, ref, fontsize=7.8, color=MUTED,
                    ha="center", va="center", family=MONO)
            ax.add_patch(FancyArrowPatch(
                (rx, circle_y + radius), (cx + sign * 0.45, pillar_y),
                arrowstyle="-|>,head_length=6,head_width=3.4",
                color=edge, linewidth=1.4, shrinkA=0, shrinkB=0, zorder=3))

        box(ax, cx - pillar_w / 2, pillar_y, pillar_w, pillar_h, fill, edge, lw=2.0)
        ax.text(cx, pillar_y + pillar_h / 2, label, fontsize=12.5, fontweight="bold",
                color=edge, ha="center", va="center")

    legend_y = 1.7
    ax.add_patch(FancyBboxPatch((4.2, legend_y - 0.24), 0.52, 0.5,
                                boxstyle="round,pad=0.03,rounding_size=0.1",
                                facecolor="#e2e8f0", edgecolor=MUTED, linewidth=1.3))
    ax.text(4.95, legend_y, "= obligation the agent inherits", fontsize=10.5,
            color=BODY, va="center")
    ax.add_patch(Circle((9.9, legend_y), 0.27, facecolor="#e2e8f0",
                        edgecolor=MUTED, linewidth=1.3))
    ax.text(10.35, legend_y, "= rule, with the system that holds the fact",
            fontsize=10.5, color=BODY, va="center")

    footer(ax, w, "Each rule is keyed off a value in the payload and carries a fact that is not in it. "
                  "None of the six systems owns the ERP API.")
    save(fig, "policy-hub.png")


# ---------------------------------------------------------------------------
# Style 2 — the contract layer, as five facets around a centre.
# ---------------------------------------------------------------------------

FACETS = [
    ("DETECTION", "when  ·  jsonpath-subset", "#5b6b80"),
    ("PROVENANCE", "owner  ·  reference id", "#7a8798"),
    ("DETERMINISM", "no model in the data path", "#4c3a7a"),
    ("INTEGRITY", "hash-pinned  ·  fail closed", "#8d99a8"),
    ("DELIVERY", "structuredContent  ·  content[]", "#66748a"),
]


def render_facets():
    w, h = 13.5, 11.6
    fig, ax = canvas(w, h)
    heading(
        ax, w, h,
        "The contract layer",
        "What a semantic contract carries that an output schema structurally cannot",
    )

    cx, cy, outer, inner = w / 2, h / 2 - 0.55, 4.15, 2.25
    angles = [90 - 72 * i for i in range(6)]
    pts = [(cx + outer * np.cos(np.radians(a)), cy + outer * np.sin(np.radians(a)))
           for a in angles]
    qts = [(cx + inner * np.cos(np.radians(a)), cy + inner * np.sin(np.radians(a)))
           for a in angles]

    for i, (name, detail, colour) in enumerate(FACETS):
        band = [pts[i], pts[i + 1], qts[i + 1], qts[i]]
        ax.add_patch(Polygon(band, closed=True, facecolor=colour,
                             edgecolor=WHITE, linewidth=2.5, zorder=2))

        mid_out = np.array(pts[i]) * 0.5 + np.array(pts[i + 1]) * 0.5
        mid_in = np.array(qts[i]) * 0.5 + np.array(qts[i + 1]) * 0.5
        mid = (mid_out + mid_in) / 2

        dx, dy = np.array(pts[i + 1]) - np.array(pts[i])
        rot = np.degrees(np.arctan2(dy, dx))
        if rot > 90:
            rot -= 180
        elif rot < -90:
            rot += 180

        ax.text(mid[0], mid[1], detail, fontsize=8.6, color=WHITE, ha="center",
                va="center", rotation=rot, rotation_mode="anchor", zorder=3)

        out = mid_out - np.array([cx, cy])
        out = out / np.linalg.norm(out)
        label = mid_out + out * 0.78
        ax.text(label[0], label[1], name, fontsize=13.5, fontweight="bold",
                color=INK, ha="center", va="center", zorder=3)

    ax.add_patch(Polygon(qts[:5], closed=True, facecolor=BG,
                         edgecolor=WHITE, linewidth=2.5, zorder=2))
    ax.text(cx, cy + 0.22, "MCP SEMANTIC", fontsize=14.5, fontweight="bold",
            color=PLUM, ha="center", va="center", zorder=3)
    ax.text(cx, cy - 0.24, "CONTRACT", fontsize=14.5, fontweight="bold",
            color=PLUM, ha="center", va="center", zorder=3)

    footer(ax, w, "Evaluated at the gateway on every tools/call result. "
                  "The upstream payload is never rewritten.")
    save(fig, "policy-facets.png")


# ---------------------------------------------------------------------------
# Style 3 — three panels: what each layer is allowed to say.
# ---------------------------------------------------------------------------

PANELS = [
    (
        "outputSchema", "a static description of the tool", AMBER, AMBER_FILL,
        [
            "deliveryId       : string",
            "estimatedArrival : date",
            "batchNumber      : string",
            "eccn             : string | null",
            "",
            "# true of every delivery,",
            "# for as long as the tool lives",
        ],
        "Authored once, with the tool.\nOwned by the team that owns the API.",
    ),
    (
        "Semantic contract", "facts that are true only right now", PLUM, PLUM_FILL,
        [
            "id       : batch-under-recall",
            "severity : critical",
            "when     : matches(items[*]",
            '             .batchNumber,',
            '             \"^B-7741-\")',
            "guidance : under recall,",
            "             do not confirm",
            "reference: QN-2026-0412",
        ],
        "Authored by the team that holds the fact.\nChanges on its own clock.",
    ),
    (
        "Injected guidance", "what the agent must do about it", GREEN, GREEN_FILL,
        [
            "structuredContent:",
            "  _semanticContract:",
            "    - critical:",
            "        do not confirm,",
            "        route to Quality",
            "",
            "# payload itself untouched",
        ],
        "Evaluated per response at the gateway.\nDelivered inside the tool result.",
    ),
]


def render_layers():
    w, h = 17.0, 9.4
    fig, ax = canvas(w, h)
    heading(
        ax, w, h,
        "Three layers, three different owners",
        "A schema says what a field is. A contract says what is true about this result, now.",
    )

    panel_w, gap = 4.9, 0.72
    left = (w - (panel_w * 3 + gap * 2)) / 2
    top = h - 2.0

    for i, (title, subtitle, edge, fill, lines, note) in enumerate(PANELS):
        x = left + i * (panel_w + gap)

        if i:
            sx = x - gap / 2
            ax.plot([sx, sx], [1.3, top - 0.1], color=RULE_LINE,
                    linewidth=1.4, linestyle=(0, (3, 4)))

        ax.text(x + panel_w / 2, top - 0.25, title, fontsize=15, fontweight="bold",
                color=edge, ha="center", va="center")
        ax.text(x + panel_w / 2, top - 0.82, subtitle, fontsize=10, color=MUTED,
                ha="center", va="center", style="italic")

        card_h = 3.35
        card_y = top - 1.35 - card_h
        box(ax, x + 0.25, card_y, panel_w - 0.5, card_h, fill, edge, lw=1.6)

        ry = card_y + card_h - 0.5
        for line in lines:
            colour = MUTED if line.startswith("#") else INK
            ax.text(x + 0.55, ry, line, fontsize=8.8, color=colour,
                    family=MONO, va="center")
            ry -= 0.42

        ax.text(x + panel_w / 2, card_y - 0.72, note, fontsize=9.6, color=BODY,
                ha="center", va="center", linespacing=1.5)

    footer(ax, w, "The policy adds only the middle and right columns. "
                  "It never edits the payload and never competes with the schema.")
    save(fig, "policy-layers.png")


# ---------------------------------------------------------------------------
# Style 4 — the board: what comes in, what the layer adds, who consumes it.
# ---------------------------------------------------------------------------

INK_BAR = "#1e293b"

UPSTREAM = [
    ("Tool result", ["structuredContent", "content[]", "isError"]),
    ("outputSchema", ["field names and types", "what a field means, always"]),
    ("Owned by", ["the team that owns the API"]),
]

LAYER_TOP = [
    ("RULES", ["id", "severity", "when", "guidance", "reference"]),
    ("CONDITIONS", ["matches(...) patterns", "items[*] quantifiers", "thresholds, dates, nulls"]),
    ("OWNERS", ["Quality", "Logistics", "Trade Compliance", "Legal, Engineering, Finance"]),
]

LAYER_ROWS = [
    ("EVALUATION", "parsed at load · evaluated per response · never errors"),
    ("INTEGRITY", "hash-pinned · fail closed · delimiters defanged"),
    ("DELIVERY", "structuredContent._semanticContract + content[]"),
]

CONSUMERS = [
    ("AI agents", GREEN, [
        "Sees obligations, not just fields",
        "Declines to promise a recalled line",
        "Will not disclose a controlled route",
    ]),
    ("Humans", BLUE, [
        "Support and ops read the same",
        "guidance the agent acted on",
    ]),
    ("Outcomes", AMBER, [
        "\u2713  No date quoted on suspended service",
        "\u2713  No customer mail on a legal hold",
        "\u2713  Payload byte-identical throughout",
    ]),
]

PUSHBACK = [
    ("\u2717 \u201cPut it in the outputSchema\u201d",
     "the schema ships with the tool; these facts change daily and other teams own them"),
    ("\u2717 \u201cLet the model work it out\u201d",
     "no model in the data path \u2014 rules are deterministic, versioned and auditable"),
    ("\u2717 \u201cThis is schema validation\u201d",
     "nothing is blocked and the payload is never rewritten; guidance is added beside it"),
]


def column_header(ax, x, y, w, text):
    box(ax, x, y, w, 0.52, INK_BAR, INK_BAR, lw=1.0, rounding=0.1)
    ax.text(x + w / 2, y + 0.26, text, fontsize=11, fontweight="bold",
            color=WHITE, ha="center", va="center", zorder=3)


def card(ax, x, y_top, w, title, lines, edge, fill, title_size=8.6, line_size=8.4):
    """Draws a titled card whose top edge is at `y_top`; returns the new y."""
    h = 0.5 + 0.3 * len(lines)
    y = y_top - h
    box(ax, x, y, w, h, fill, edge, lw=1.2, rounding=0.1)
    ax.text(x + 0.22, y + h - 0.26, title, fontsize=title_size, fontweight="bold",
            color=edge, va="center", zorder=3)
    for i, line in enumerate(lines):
        ax.text(x + 0.22, y + h - 0.58 - 0.3 * i, line, fontsize=line_size,
                color=BODY, va="center", zorder=3)
    return y - 0.22


def render_board():
    w, h = 19.0, 10.2
    fig, ax = canvas(w, h)

    ax.text(w / 2, h - 0.55, "One contract, every consumer", fontsize=21,
            fontweight="bold", color=INK, ha="center", va="center")
    ax.text(w / 2, h - 1.12,
            "MCP tool result  \u2192  rules evaluated at the gateway  \u2192  agents that "
            "know what they may not say",
            fontsize=11, color=MUTED, ha="center", va="center")

    top = h - 1.95
    lx, lw_ = 0.45, 3.95
    mx, mw = 5.35, 8.25
    rx, rw = 14.55, 4.0

    # The layer, sized to its own content ---------------------------------
    card_h = max(0.5 + 0.3 * len(lines) for _, lines in LAYER_TOP)
    rows_top = top - 0.3 - card_h - 0.22
    rows_bottom = rows_top - 0.78 * len(LAYER_ROWS)
    panel_bottom = rows_bottom - 0.62
    box(ax, mx, panel_bottom, mw, top + 0.52 - panel_bottom, PLUM_FILL, PLUM, lw=2.0)
    column_header(ax, mx, top, mw, "SEMANTIC CONTRACT LAYER")

    inner_w = (mw - 0.6) / 3
    for i, (title, lines) in enumerate(LAYER_TOP):
        card(ax, mx + 0.15 + i * (inner_w + 0.15), top - 0.3,
             inner_w, title, lines, PLUM, WHITE)

    y = rows_top
    for title, detail in LAYER_ROWS:
        box(ax, mx + 0.15, y - 0.62, mw - 0.3, 0.62, WHITE, PLUM, lw=1.2, rounding=0.1)
        ax.text(mx + 0.35, y - 0.31, title, fontsize=8.4, fontweight="bold",
                color=PLUM, va="center", zorder=3)
        ax.text(mx + 1.85, y - 0.31, detail, fontsize=8.0, color=BODY,
                va="center", zorder=3, family=MONO)
        y -= 0.78

    ax.text(mx + mw / 2, panel_bottom + 0.31,
            "\u201cWhat is true about this result, right now?\u201d",
            fontsize=9.5, color=PLUM, ha="center", va="center", style="italic")

    # Upstream ------------------------------------------------------------
    column_header(ax, lx, top, lw_, "UPSTREAM MCP TOOL")
    y = top - 0.3
    for title, lines in UPSTREAM:
        y = card(ax, lx + 0.15, y, lw_ - 0.3, title, lines, AMBER, AMBER_FILL)
    ax.text(lx + lw_ / 2, y - 0.15, "\u201cHow is this document shaped?\u201d",
            fontsize=9, color=MUTED, ha="center", va="center", style="italic")

    # Consumers -----------------------------------------------------------
    column_header(ax, rx, top, rw, "CONSUMERS")
    y = top - 0.3
    for title, edge, lines in CONSUMERS:
        fill = {GREEN: GREEN_FILL, BLUE: BLUE_FILL, AMBER: AMBER_FILL}[edge]
        y = card(ax, rx + 0.15, y, rw - 0.3, title, lines, edge, fill)

    for y_arrow, label, x0, x1 in (
        (top - 1.15, "govern", lx + lw_, mx),
        (top - 1.15, "guidance", mx + mw, rx),
    ):
        ax.add_patch(FancyArrowPatch(
            (x0 + 0.06, y_arrow), (x1 - 0.06, y_arrow),
            arrowstyle="-|>,head_length=6,head_width=3.4",
            color=PLUM, linewidth=1.6, shrinkA=0, shrinkB=0, zorder=4))
        ax.text((x0 + x1) / 2, y_arrow + 0.24, label, fontsize=7.8, color=PLUM,
                ha="center", va="center")

    # Pushback ------------------------------------------------------------
    band_y, band_h = 0.75, 1.55
    box(ax, 0.45, band_y, w - 0.9, band_h, "#2b1f3d", "#2b1f3d", lw=1.0, rounding=0.12)
    ax.text(0.8, band_y + band_h - 0.3, "Common pushback \u2014 and why it does not hold",
            fontsize=10, fontweight="bold", color="#e9d8fd", va="center", zorder=3)
    for i, (claim, answer) in enumerate(PUSHBACK):
        yy = band_y + band_h - 0.68 - 0.32 * i
        ax.text(0.8, yy, claim, fontsize=8.6, color="#f7ecff", va="center", zorder=3)
        ax.text(6.0, yy, answer, fontsize=8.6, color="#c4b5d8", va="center", zorder=3)

    save(fig, "policy-board.png")


# ---------------------------------------------------------------------------
# Style 5 — the domain model, as labelled relations.
# ---------------------------------------------------------------------------

NODES = {
    "contract": (5.5, 9.0, "Semantic\nContract"),
    "tool": (2.0, 6.6, "MCP\nTool"),
    "result": (5.5, 6.6, "Tool\nResult"),
    "rule": (9.3, 6.6, "Rule"),
    "owner": (12.9, 9.0, "System of\nRecord"),
    "guidance": (12.9, 4.6, "Guidance"),
    "payload": (5.5, 3.0, "Payload"),
    "condition": (9.3, 3.0, "when\nCondition"),
    "agent": (2.0, 3.0, "AI\nAgent"),
}

# (from, to, label, bow, label nudge x, label nudge y)
EDGES = [
    ("contract", "tool", "governs", 0.0, 0.0, 0.0),
    ("contract", "rule", "defines", 0.0, 0.0, 0.0),
    ("tool", "result", "produces", 0.0, 0.0, 0.0),
    ("result", "payload", "carries", 0.0, 0.0, 0.0),
    ("rule", "condition", "tested by", 0.0, 0.62, -0.35),
    ("condition", "payload", "reads", 0.0, 0.0, 0.0),
    ("rule", "guidance", "carries", 0.0, 0.0, 0.0),
    ("owner", "guidance", "asserts", 0.0, 0.0, 0.0),
    ("guidance", "result", "attached to", -0.25, -1.15, 0.1),
    ("result", "agent", "consumed by", 0.0, 0.0, 0.0),
]


def render_ontology():
    w, h = 15.5, 11.0
    fig, ax = canvas(w, h)
    heading(
        ax, w, h,
        "The domain model",
        "A rule is tested against the payload, but the fact it carries is asserted elsewhere",
    )

    r = 0.62
    for key, (x, y, label) in NODES.items():
        accent = PLUM if key in ("contract", "rule") else INK
        fill = PLUM_FILL if key in ("contract", "rule") else WHITE
        ax.add_patch(Circle((x, y), r, facecolor=fill, edgecolor=accent,
                            linewidth=1.6, zorder=3))
        ax.text(x, y, label, fontsize=8.6, color=accent, ha="center",
                va="center", zorder=4, linespacing=1.35,
                fontweight="bold" if key in ("contract", "rule") else "normal")

    for a, b, label, curve, ndx, ndy in EDGES:
        (x0, y0, _), (x1, y1, _) = NODES[a], NODES[b]
        dx, dy = x1 - x0, y1 - y0
        dist = np.hypot(dx, dy)
        ux, uy = dx / dist, dy / dist
        p0 = (x0 + ux * r, y0 + uy * r)
        p1 = (x1 - ux * r, y1 - uy * r)
        ax.add_patch(FancyArrowPatch(
            p0, p1, arrowstyle="-|>,head_length=7,head_width=3.6",
            connectionstyle=f"arc3,rad={curve}",
            color=MUTED, linewidth=1.3, shrinkA=0, shrinkB=0, zorder=2))

        # arc3 bows toward the right-hand normal for a positive rad, and the
        # label has to follow the bow rather than the straight chord.
        # A quadratic bow reaches half way to its control point at the midpoint.
        mx_, my_ = (x0 + x1) / 2, (y0 + y1) / 2
        mx_ += uy * curve * dist * 0.25
        my_ -= ux * curve * dist * 0.25
        ax.text(mx_ + ndx, my_ + 0.16 + ndy, label, fontsize=8.4, color=BODY,
                ha="center", va="center", zorder=4,
                bbox=dict(boxstyle="round,pad=0.16", facecolor=BG, edgecolor="none"))

    ax.text(w / 2, 1.45,
            "The asymmetry is the whole policy: the condition is answerable from the "
            "document,\nwhile the guidance is not \u2014 it is asserted by a team that "
            "does not own the API.",
            fontsize=9.5, color=MUTED, ha="center", va="center", linespacing=1.5)

    footer(ax, w, "One contract governs one or more tools. One rule carries exactly one "
                  "obligation, from exactly one system of record.")
    save(fig, "policy-ontology.png")


if __name__ == "__main__":
    render_hub()
    render_facets()
    render_layers()
    render_board()
    render_ontology()
