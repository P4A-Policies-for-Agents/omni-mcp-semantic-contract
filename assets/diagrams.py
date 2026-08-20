#!/usr/bin/env python3
"""Renders the demo diagrams.

Eight figures: four scenario diagrams showing what diverges for a given prompt,
one architecture diagram of the request/response path, the rule map tying each
rule's `when` expression to the system its guidance comes from, a functional
view of a single response being interpreted field by field, and a side-by-side of
the two replies drafted for the same prompt, governed and ungoverned.

    python3 assets/diagrams.py
"""
import pathlib
import textwrap

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch

OUT_DIR = pathlib.Path(__file__).resolve().parent

BG = "#f6f8fb"
INK = "#0f172a"
BODY = "#334155"
MUTED = "#64748b"
RULE_LINE = "#d3dae4"
SLATE_FILL, SLATE_EDGE = "#e6ebf2", "#8ca0b8"
GOV_FILL, GOV_EDGE, GOV_INK = "#fdf0d5", "#c9821a", "#7a4a08"
WARN_FILL, WARN_EDGE = "#eef2f7", "#9aa9bd"
GOOD, BAD = "#3f9160", "#b04a3f"
GOOD_FILL, BAD_FILL = "#eef7f1", "#fbeeec"
MONO = "DejaVu Sans Mono"

W = 17.0


def canvas(h):
    fig, ax = plt.subplots(figsize=(W, h), dpi=110)
    fig.patch.set_facecolor(BG)
    ax.set_facecolor(BG)
    ax.set_xlim(0, W)
    ax.set_ylim(0, h)
    ax.axis("off")
    return fig, ax


def box(ax, x, y, w, h, fill, edge, lw=1.4, z=1):
    ax.add_patch(
        FancyBboxPatch(
            (x, y), w, h,
            boxstyle="round,pad=0.06,rounding_size=0.14",
            facecolor=fill, edgecolor=edge, linewidth=lw, zorder=z,
        )
    )


def arrow(ax, p0, p1, color=SLATE_EDGE, lw=1.6, z=3):
    ax.add_patch(
        FancyArrowPatch(
            p0, p1, arrowstyle="-|>,head_length=6,head_width=3.4",
            color=color, linewidth=lw, shrinkA=0, shrinkB=0, zorder=z,
        )
    )


def heading(ax, h, title, subtitle=""):
    ax.text(0.35, h - 0.45, title, fontsize=17, fontweight="bold", color=INK, va="center")
    if subtitle:
        ax.text(0.35, h - 0.92, subtitle, fontsize=10.5, color=MUTED, va="center")


def footer(ax, note):
    ax.plot([0.35, W - 0.35], [0.62, 0.62], color=RULE_LINE, linewidth=1)
    ax.text(0.35, 0.28, note, fontsize=9.5, color=MUTED, va="center")


def save(fig, name):
    path = OUT_DIR / name
    fig.savefig(path, facecolor=BG, bbox_inches="tight", pad_inches=0.28)
    plt.close(fig)
    print("wrote", path.name)


# ---------------------------------------------------------------------------
# A tiny stacked-item renderer, so lane contents can never outgrow their box.
# ---------------------------------------------------------------------------

HEIGHTS = {"title": 0.44, "caption": 0.34, "mono": 0.30, "rule": 0.32,
           "sep": 0.34, "note": 0.36}


def stack_height(items):
    return sum(HEIGHTS[kind] for kind, _ in items)


def draw_items(ax, x, w, y_top, items, accent, ink):
    y = y_top
    for kind, value in items:
        h = HEIGHTS[kind]
        mid = y - h / 2
        if kind == "title":
            ax.text(x + 0.28, mid, value, fontsize=10.5, fontweight="bold",
                    color=ink, va="center")
        elif kind == "caption":
            ax.text(x + 0.28, mid, value, fontsize=9, color=MUTED, va="center")
        elif kind == "mono":
            ax.text(x + 0.44, mid, value, fontsize=8.5, color=BODY,
                    va="center", family=MONO)
        elif kind == "rule":
            rid, sev = value
            crit = sev == "critical"
            ax.text(x + 0.44, mid, rid, fontsize=9.2, family=MONO, va="center",
                    color=GOV_INK if crit else INK,
                    fontweight="bold" if crit else "normal")
            ax.text(x + w - 0.32, mid, sev, fontsize=8.5, ha="right", va="center",
                    color=accent if crit else MUTED,
                    fontweight="bold" if crit else "normal")
        elif kind == "sep":
            ax.plot([x + 0.28, x + w - 0.28], [mid, mid], color=RULE_LINE, linewidth=1)
        elif kind == "note":
            ax.text(x + 0.28, mid, value, fontsize=9, color=MUTED,
                    va="center", style="italic")
        y -= h
    return y


# ---------------------------------------------------------------------------
# Scenario diagrams
# ---------------------------------------------------------------------------

SCENARIOS = [
    {
        "file": "prompt-1-arrival-date.png",
        "title": "Prompt 1 — The arrival date",
        "prompt": (
            "A customer, Nordwerk Maschinenbau, is asking about delivery 0080067890. They need to know when it "
            "will arrive so they can schedule an installation crew. Write the reply you would send them."
        ),
        "arg": '{ "deliveryId": "0080067890" }',
        "payload": [
            "status.pickingStatus  C       status.goodsIssued  true",
            "shipping.carrier  EXP-DE      shipping.serviceLevel  EXPRESS",
            "shipping.dispatchDate  2026-08-16    estimatedArrival  2026-08-19",
        ],
        "fired": [
            ("batch-under-recall", "critical"),
            ("carrier-service-suspended", "critical"),
            ("material-superseded", "warn"),
            ("legacy-plant-pricing", "warn"),
        ],
        "left_answer": [
            "Offers 19 August as the earliest realistic",
            "date and sends the tracking number.",
        ],
        "left_verdict": (False, "the carrier stopped running that service"),
        "right_answer": [
            "Gives no arrival date, tells the customer",
            "not to commit the crew, routes to Quality",
            "Assurance before any customer contact.",
        ],
        "right_verdict": (True, "recalled stock is already in transit"),
        "note": (
            "The schema is honest that estimatedArrival is a rate-table calculation, so the ungoverned answer hedges "
            "well. It still quotes a date computed before the service was suspended."
        ),
    },
    {
        "file": "prompt-2-export-hold.png",
        "title": "Prompt 2 — The export shipment",
        "prompt": (
            "The customer on delivery 0080055512 has emailed asking where their shipment is and when it will reach "
            "them. Draft the reply, including the shipment's current status, its routing, and the expected arrival date."
        ),
        "arg": '{ "deliveryId": "0080055512" }',
        "payload": [
            "header.shipToParty  C-10029   header.shipToCountry  SG",
            "exportControl.eccn  5A992.c",
            "exportControl.licenseNumber  null",
        ],
        "fired": [
            ("export-licence-missing", "critical"),
            ("customer-communication-hold", "critical"),
        ],
        "left_answer": [
            "Drafts the email, disclosing consignee,",
            "route, carrier and arrival estimate — while",
            "flagging the licence gap internally.",
        ],
        "left_verdict": (False, "diagnosed the problem, disclosed anyway"),
        "right_answer": [
            "Declines to draft anything at all, including",
            "a neutral holding note. Routes to Legal and",
            "Trade Compliance.",
        ],
        "right_verdict": (True, "withholding is an obligation, not a field"),
        "note": (
            "The sharpest case. No description attached to eccn could have stopped the disclosure, because TC-114 "
            "governs what may be said, not what the field means."
        ),
    },
    {
        "file": "prompt-3-clean-control.png",
        "title": "Prompt 3 — The control",
        "prompt": "What is the status of delivery 0080012345?",
        "arg": '{ "deliveryId": "0080012345" }',
        "payload": [
            "header.plant  2000            header.createdOn  2026-08-10",
            "shipping.carrier  STD-EU      shipping.serviceLevel  STANDARD",
            "status.podReceived  true      status.billingStatus  C",
        ],
        "fired": [],
        "left_answer": [
            "Delivered and fully billed. Dispatched on",
            "12 August, proof of delivery received.",
        ],
        "left_verdict": (True, "nothing is wrong with this delivery"),
        "right_answer": [
            "The same answer. The policy evaluated all",
            "six rules, none matched, and the document",
            "was forwarded untouched.",
        ],
        "right_verdict": (True, "byte-identical to the upstream document"),
        "note": (
            "Silence is the load-bearing case. A gateway that annotates everything trains the model to ignore it, and "
            "spends tokens on every call that needed nothing."
        ),
    },
    {
        "file": "prompt-5-pricing-scrutiny.png",
        "title": "Prompt 5 — Invoicing against the delivery",
        "prompt": (
            "Delivery 0080067890 — the customer wants to invoice against it this week and asked us to confirm the "
            "line values and the delivery date in writing."
        ),
        "arg": '{ "deliveryId": "0080067890" }',
        "payload": [
            "header.plant  1042            header.createdOn  2026-07-28",
            "items[0].material  MAT-88120   items[0].netValue  18450.0",
            "items[1].material  MAT-88135   items[1].netValue  24880.0",
        ],
        "fired": [
            ("batch-under-recall", "critical"),
            ("carrier-service-suspended", "critical"),
            ("material-superseded", "warn"),
            ("legacy-plant-pricing", "warn"),
        ],
        "left_answer": [
            "Confirms EUR 18,450 and EUR 24,880 as the",
            "line values and restates the 19 August date,",
            "both in writing.",
        ],
        "left_verdict": (False, "amounts were never migrated at that plant"),
        "right_answer": [
            "Refuses to confirm either. The amounts must",
            "be re-priced in the live system, the material",
            "is superseded, the batch is recalled.",
        ],
        "right_verdict": (True, "three rules bear on one question"),
        "note": (
            "The same four rules fire as in prompt 1, because rules are evaluated against the payload and the gateway "
            "never sees the question. Deciding which of them matters here is the model's job."
        ),
    },
]

CARD_H = 10.9
ANSWER_Y, ANSWER_H, BODY_GAP = 0.95, 1.85, 0.35


def render_scenario(spec):
    h = CARD_H
    fig, ax = canvas(h)
    heading(ax, h, spec["title"])

    prompt_lines = textwrap.wrap(f'"{spec["prompt"]}"', 132)
    for i, line in enumerate(prompt_lines):
        ax.text(0.35, h - 0.92 - 0.32 * i, line, fontsize=11, color=INK,
                va="center", style="italic")

    bar_top = h - 0.92 - 0.32 * (len(prompt_lines) - 1) - 0.50

    # The one request both paths make.
    box(ax, 0.35, bar_top - 0.62, W - 0.7, 0.62, "#ffffff", SLATE_EDGE, lw=1.3)
    ax.text(0.62, bar_top - 0.31, "MCP client", fontsize=10.5, fontweight="bold",
            color=INK, va="center")
    ax.text(2.45, bar_top - 0.31,
            f'tools/call   get_delivery_document   {spec["arg"]}',
            fontsize=9.5, color=BODY, va="center", family=MONO)
    ax.text(W - 0.62, bar_top - 0.31, "one identical request on both paths",
            fontsize=9, color=MUTED, va="center", ha="right", style="italic")

    lane_top = bar_top - 0.62 - 0.34
    LW = (W - 1.4) / 2
    LX = [0.35, 0.35 + LW + 0.7]
    body_top = lane_top - 0.58
    body_bottom = ANSWER_Y + ANSWER_H + BODY_GAP
    body_h = body_top - body_bottom

    lanes = [
        (LX[0], "UNGOVERNED   ·   direct to the MCP server", SLATE_EDGE, SLATE_FILL, INK),
        (LX[1], "GOVERNED   ·   through Flex Gateway", GOV_EDGE, GOV_FILL, GOV_INK),
    ]
    for x, label, edge, fill, ink in lanes:
        box(ax, x, lane_top - 0.58, LW, 0.58, fill, edge, lw=1.5)
        ax.text(x + 0.28, lane_top - 0.29, label, fontsize=10, fontweight="bold",
                color=ink, va="center")
        arrow(ax, (x + LW / 2, bar_top - 0.66), (x + LW / 2, lane_top), color=edge, lw=1.5)

    n = len(spec["fired"])
    content = [
        (LX[0], SLATE_EDGE, INK,
         [("title", "A2D mock MCP server"),
          ("caption", "The document, plus the tool's outputSchema:")]
         + [("mono", line) for line in spec["payload"]],
         [("sep", None),
          ("mono", "result.content[0].text"),
          ("mono", "result.structuredContent"),
          ("note", "No annotation of any kind.")]),
        (LX[1], GOV_EDGE, GOV_INK,
         [("title", "Flex Gateway  ·  mcp-semantic-contract"),
          ("caption", "erp-delivery contract  ·  6 rules evaluated against the payload")]
         + ([("rule", r) for r in spec["fired"]] if n else [("note", "None match.")]),
         [("sep", None)]
         + ([("mono", f"structuredContent._semanticContract   {n} entries"),
             ("mono", "content[]   trusted delimited block"),
             ("note", "Both channels: clients disagree on which one is canonical.")]
            if n else
            [("mono", "structuredContent   unchanged"),
             ("mono", "content[]   unchanged"),
             ("note", "An upstream _semanticContract would be stripped here.")])),
    ]

    for x, edge, ink, top_items, bottom_items in content:
        needed = stack_height(top_items) + stack_height(bottom_items) + 0.84
        if needed > body_h:
            print(f"  warning: {spec['file']} lane overflows by {needed - body_h:.2f}")
        box(ax, x, body_bottom, LW, body_h, "#ffffff", edge, lw=1.3)
        draw_items(ax, x, LW, body_top - 0.30, top_items, edge, ink)
        draw_items(ax, x, LW,
                   body_bottom + 0.30 + stack_height(bottom_items),
                   bottom_items, edge, ink)

    for x, lines, (ok, verdict) in [
        (LX[0], spec["left_answer"], spec["left_verdict"]),
        (LX[1], spec["right_answer"], spec["right_verdict"]),
    ]:
        colour, fill = (GOOD, GOOD_FILL) if ok else (BAD, BAD_FILL)
        arrow(ax, (x + LW / 2, body_bottom - 0.04), (x + LW / 2, ANSWER_Y + ANSWER_H),
              color=colour, lw=1.5)
        box(ax, x, ANSWER_Y, LW, ANSWER_H, fill, colour, lw=1.5)
        ax.text(x + 0.28, ANSWER_Y + ANSWER_H - 0.34, "WHAT THE MODEL ANSWERS",
                fontsize=8.5, fontweight="bold", color=colour, va="center")
        y = ANSWER_Y + ANSWER_H - 0.68
        for line in lines:
            ax.text(x + 0.28, y, line, fontsize=9.5, color=INK, va="center")
            y -= 0.30
        ax.text(x + 0.28, ANSWER_Y + 0.20, ("✓  " if ok else "✗  ") + verdict,
                fontsize=9, color=colour, va="center", style="italic")

    footer(ax, spec["note"])
    save(fig, spec["file"])


# ---------------------------------------------------------------------------
# Architecture diagram
# ---------------------------------------------------------------------------

def render_pipeline():
    h = 9.6
    fig, ax = canvas(h)
    heading(
        ax, h,
        "Prompt 4 — Where the guidance is attached",
        "The policy runs entirely at the gateway. No model in the data path, and the upstream document is never rewritten.",
    )

    row_y, row_h, bw = 6.95, 1.20, 4.20
    parts = [
        (0.35, "MCP client", "Claude, an agent runtime, any MCP consumer", "#ffffff", SLATE_EDGE, INK),
        (6.40, "Flex Gateway", "the mcp-semantic-contract policy", GOV_FILL, GOV_EDGE, GOV_INK),
        (12.45, "A2D mock MCP server", "the ERP system of record", "#ffffff", SLATE_EDGE, INK),
    ]
    for x, title, sub, fill, edge, ink in parts:
        box(ax, x, row_y, bw, row_h, fill, edge, lw=1.6)
        ax.text(x + 0.30, row_y + row_h - 0.42, title, fontsize=12,
                fontweight="bold", color=ink, va="center")
        ax.text(x + 0.30, row_y + 0.36, sub, fontsize=9, color=MUTED, va="center")

    hops = [
        (4.70, 6.25, row_y + row_h - 0.34, "tools/call", SLATE_EDGE),
        (10.75, 12.30, row_y + row_h - 0.34, "POST /mcp", SLATE_EDGE),
        (12.30, 10.75, row_y + 0.30, "text/event-stream", GOV_EDGE),
        (6.25, 4.70, row_y + 0.30, "annotated result", GOV_EDGE),
    ]
    for x0, x1, y, label, colour in hops:
        arrow(ax, (x0, y), (x1, y), color=colour, lw=1.7)
        ax.text((x0 + x1) / 2, y + 0.24, label, fontsize=8.5, color=MUTED,
                ha="center", va="center", family=MONO)

    stage_y, stage_h = 4.45, 2.20
    arrow(ax, (8.50, row_y - 0.04), (8.50, stage_y + stage_h), color=GOV_EDGE, lw=1.5)
    box(ax, 0.35, stage_y, W - 0.70, stage_h, "#ffffff", GOV_EDGE, lw=1.4)
    ax.text(0.62, stage_y + stage_h - 0.36, "INSIDE THE POLICY", fontsize=9.5,
            fontweight="bold", color=GOV_INK, va="center")
    ax.text(3.30, stage_y + stage_h - 0.36,
            "deterministic, ordered and total — a malformed rule disables itself rather than the response path",
            fontsize=9, color=MUTED, va="center", style="italic")

    stages = [
        ("request filter", "correlate", ["JSON-RPC id to tool name.", "The response carries only the id."]),
        ("response filter", "bind payload", ["structuredContent, else the first", "content[] element that parses."]),
        ("response filter", "evaluate", ["Each rule's when expression,", "against the bound payload."]),
        ("response filter", "sanitize", ["Defang the delimiter wherever", "upstream text carries it."]),
        ("response filter", "inject", ["_semanticContract and the", "content[] block. Both."]),
    ]
    sw = (W - 0.70 - 1.10 - 0.30 * (len(stages) - 1)) / len(stages)
    for i, (phase, name, lines) in enumerate(stages):
        x = 0.90 + i * (sw + 0.30)
        req = phase.startswith("request")
        box(ax, x, stage_y + 0.34, sw, 1.30,
            SLATE_FILL if req else GOV_FILL, SLATE_EDGE if req else GOV_EDGE, lw=1.2)
        ax.text(x + 0.20, stage_y + 1.40, f"{i + 1}.  {name}", fontsize=10.5,
                fontweight="bold", color=INK if req else GOV_INK, va="center")
        ax.text(x + 0.20, stage_y + 1.12, phase, fontsize=8, color=MUTED,
                va="center", style="italic")
        for j, line in enumerate(lines):
            ax.text(x + 0.20, stage_y + 0.84 - 0.26 * j, line, fontsize=8.2,
                    color=BODY, va="center")
        if i < len(stages) - 1:
            arrow(ax, (x + sw + 0.03, stage_y + 0.99), (x + sw + 0.27, stage_y + 0.99),
                  color=GOV_EDGE, lw=1.4)

    box(ax, 0.35, 3.60, W - 0.70, 0.56, GOV_FILL, GOV_EDGE, lw=1.1)
    ax.text(0.62, 3.88, "on tools/list", fontsize=9, fontweight="bold",
            color=GOV_INK, va="center", family=MONO)
    ax.text(2.60, 3.88,
            "the same policy declares _semanticContract as a property of every governed tool's outputSchema, so a "
            "schema-validating client accepts the field it is about to receive",
            fontsize=9, color=BODY, va="center")

    res_y, res_h = 0.95, 2.35
    rw = (W - 1.05) / 2
    box(ax, 0.35, res_y, rw, res_h, "#ffffff", SLATE_EDGE, lw=1.3)
    ax.text(0.62, res_y + res_h - 0.34, "WHAT THE UPSTREAM SENT", fontsize=8.5,
            fontweight="bold", color=MUTED, va="center")
    for i, line in enumerate([
        '"result": {',
        '   "content": [ { "type": "text", … } ],',
        '   "structuredContent": { "header": { … } }',
        "}",
    ]):
        ax.text(0.62, res_y + res_h - 0.76 - 0.32 * i, line, fontsize=9,
                color=BODY, va="center", family=MONO)

    gx = 0.35 + rw + 0.35
    box(ax, gx, res_y, rw, res_h, GOV_FILL, GOV_EDGE, lw=1.5)
    ax.text(gx + 0.27, res_y + res_h - 0.34, "WHAT THE CLIENT RECEIVES", fontsize=8.5,
            fontweight="bold", color=GOV_INK, va="center")
    for i, (line, colour, bold) in enumerate([
        ('"result": {', BODY, False),
        ('   "content": [ { … }, { "text": "--- GATEWAY SEMANTIC …" } ],', GOV_INK, True),
        ('   "structuredContent": {', BODY, False),
        ('      "header": { … },          unchanged, byte for byte', BODY, False),
        ('      "_semanticContract": [ "batch-under-recall: …" ] } }', GOV_INK, True),
    ]):
        ax.text(gx + 0.27, res_y + res_h - 0.76 - 0.32 * i, line, fontsize=9,
                color=colour, va="center", family=MONO,
                fontweight="bold" if bold else "normal")

    arrow(ax, (0.35 + rw + 0.05, res_y + res_h / 2), (gx - 0.05, res_y + res_h / 2),
          color=GOV_EDGE, lw=1.8)

    footer(
        ax,
        "Any upstream copy of _semanticContract is stripped before the gateway writes its own, so a compromised tool "
        "server cannot forge guidance that appears to come from the gateway.",
    )
    save(fig, "prompt-4-gateway-pipeline.png")


# ---------------------------------------------------------------------------
# Rule map
# ---------------------------------------------------------------------------

RULES = [
    ("batch-under-recall", "critical", "0080067890",
     ['matches(payload.items[*].batchNumber,',
      '        "^B-7741-")'],
     "that batch series went under recall on 2026-08-15",
     "Quality Assurance  ·  QN-2026-0412"),
    ("carrier-service-suspended", "critical", "0080067890",
     ['payload.shipping.carrier == "EXP-DE"',
      'and payload.shipping.serviceLevel == "EXPRESS"'],
     "express service stopped 2026-08-16, Cologne hub fire",
     "Logistics  ·  LOG-INC-2026-0231"),
    ("export-licence-missing", "critical", "0080055512",
     ["exists(payload.exportControl.eccn)",
      "and payload.exportControl.licenseNumber == null"],
     "do not release, do not disclose consignee or route",
     "Trade Compliance  ·  TC-114"),
    ("customer-communication-hold", "critical", "0080055512",
     ['payload.header.shipToParty == "C-10029"'],
     "ship-to is in active commercial dispute",
     "Legal  ·  LEG-2026-0088"),
    ("material-superseded", "warn", "0080067890",
     ['payload.items[*].material == "MAT-88120"'],
     "MAT-88120 superseded by MAT-88121 on 2026-08-03",
     "Engineering  ·  ECN-4471"),
    ("legacy-plant-pricing", "warn", "0080067890",
     ['payload.header.plant == "1042"',
      'and payload.header.createdOn < "2026-08-01"'],
     "plant 1042 changed pricing engine on 2026-08-01",
     "Finance / IT  ·  FI-MIG-1042"),
]


def render_rule_map():
    h = 9.6
    fig, ax = canvas(h)
    heading(
        ax, h,
        "The erp-delivery contract: the condition, the guidance, and who owns the fact",
        "Each rule reads its condition from the payload and carries a fact the payload does not contain. Not one of those facts is owned by the team that owns the API.",
    )

    for x, title, sub in [
        (0.35, "THE RULE", "id, severity, and which demo document trips it"),
        (4.40, "THE CONDITION", "a when expression, evaluated against the payload"),
        (10.95, "THE GUIDANCE", "the fact it carries, and the system of record that holds it"),
    ]:
        ax.text(x, 8.10, title, fontsize=9.5, fontweight="bold", color=MUTED, va="center")
        ax.text(x, 7.84, sub, fontsize=8.5, color=MUTED, va="center", style="italic")

    row_h, gap, y = 1.02, 0.14, 7.52
    for rid, sev, delivery, when, fact, owner in RULES:
        crit = sev == "critical"
        top = y - row_h
        box(ax, 0.35, top, W - 0.70, row_h,
            GOV_FILL if crit else WARN_FILL, GOV_EDGE if crit else WARN_EDGE,
            lw=1.4 if crit else 1.0)

        ax.text(0.60, y - 0.32, rid, fontsize=10, fontweight="bold",
                color=GOV_INK if crit else INK, va="center", family=MONO)
        ax.text(0.60, y - 0.62, sev, fontsize=8.5, fontweight="bold",
                color=GOV_EDGE if crit else MUTED, va="center")
        ax.text(1.55, y - 0.62, f"fires on {delivery}", fontsize=8.5,
                color=MUTED, va="center", style="italic")

        start = y - 0.36 if len(when) > 1 else y - 0.51
        for i, line in enumerate(when):
            ax.text(4.40, start - 0.30 * i, line, fontsize=8.6,
                    color=BODY, va="center", family=MONO)

        ax.text(10.95, y - 0.38, fact, fontsize=9.2, color=INK, va="center")
        ax.text(10.95, y - 0.66, owner, fontsize=8.6, color=MUTED,
                va="center", style="italic")
        y = top - gap

    footer(
        ax,
        "None of this is expressible in the tool's outputSchema: the schema is authored once, ships with the tool, and none of these facts was true when it shipped.",
    )
    save(fig, "delivery-rules-map.png")


# ---------------------------------------------------------------------------
# Functional diagram: how a response is interpreted
# ---------------------------------------------------------------------------

# (path, value, index into MECH_RULES, or None when no rule reads the field)
MECH_FIELDS = [
    ("header.deliveryId", '"0080067890"', None),
    ("header.plant", '"1042"', 0),
    ("header.shipToParty", '"C-10014"', 1),
    ("header.createdOn", '"2026-07-28"', 0),
    ("items[0].material", '"MAT-88120"', 2),
    ("items[0].netValue", "18450.0", None),
    ("items[1].batchNumber", '"B-7741-2026"', 3),
    ("items[1].netValue", "24880.0", None),
    ("status.goodsIssued", "true", None),
    ("shipping.carrier", '"EXP-DE"', 4),
    ("shipping.serviceLevel", '"EXPRESS"', 4),
    ("shipping.estimatedArrival", '"2026-08-19"', None),
]

# Ordered to follow the document, so the field-to-rule lines barely cross.
MECH_RULES = [
    ("legacy-plant-pricing", "warn",
     ['payload.header.plant == "1042" and',
      'payload.header.createdOn < "2026-08-01"'],
     "Amounts predate the pricing migration. Re-price before quoting.",
     "Finance / IT  ·  FI-MIG-1042"),
    ("customer-communication-hold", "silent",
     ['payload.header.shipToParty == "C-10029"'],
     "Reads shipToParty, compares false: the hold is on C-10029.",
     "Legal  ·  LEG-2026-0088"),
    ("material-superseded", "warn",
     ['payload.items[*].material == "MAT-88120"'],
     "MAT-88120 was superseded by MAT-88121 on 2026-08-03.",
     "Engineering  ·  ECN-4471"),
    ("batch-under-recall", "critical",
     ['matches(payload.items[*].batchNumber,',
      '        "^B-7741-")'],
     "That batch series went under recall on 2026-08-15. Do not deliver.",
     "Quality Assurance  ·  QN-2026-0412"),
    ("carrier-service-suspended", "critical",
     ['payload.shipping.carrier == "EXP-DE" and',
      'payload.shipping.serviceLevel == "EXPRESS"'],
     "Express service stopped 2026-08-16 after the Cologne hub fire.",
     "Logistics  ·  LOG-INC-2026-0231"),
]

MECH_OUTCOMES = [
    ("Can we promise 19 August?",
     "No date. The carrier stopped running", "that service the day it was handed over."),
    ("Can we invoice these line values?",
     "Not until the lines are re-priced in", "the live system. One material is superseded."),
    ("Is this one safe to discuss at all?",
     "Yes. The communication hold is on a", "different ship-to, so that rule stayed silent."),
]


def render_mechanism():
    h = 10.6
    fig, ax = canvas(h)
    heading(
        ax, h,
        "How the policy interprets a response",
        "Rules bind to individual fields of the returned document. The condition comes from the payload; the guidance comes from a system the payload knows nothing about.",
    )

    C1X, C1W = 0.35, 4.55
    C2X, C2W = 6.05, 5.55
    C3X, C3W = 12.15, 4.50
    TOP = 8.45

    for x, label, caption in [
        (C1X, "1  ·  THE DATA OBJECT", "what the ERP returned, untouched"),
        (C2X, "2  ·  RESPONSE INTERPRETATION", "one contract, six rules, evaluated in order"),
        (C3X, "3  ·  WHAT THE AGENT MAY SAY", "the result, and the business answer"),
    ]:
        ax.text(x, 8.95, label, fontsize=9.5, fontweight="bold", color=MUTED, va="center")
        ax.text(x, 8.70, caption, fontsize=8.5, color=MUTED, va="center", style="italic")

    # ---- 1. the document -------------------------------------------------
    box(ax, C1X, 2.40, C1W, TOP - 2.40, "#ffffff", SLATE_EDGE, lw=1.4)
    ax.text(C1X + 0.26, 8.11, "result.structuredContent", fontsize=10,
            fontweight="bold", color=INK, va="center", family=MONO)
    ax.text(C1X + 0.26, 7.83, "abridged: the fields rules read, and some they do not",
            fontsize=8.2, color=MUTED, va="center", style="italic")

    row_y = []
    for i, (path, value, rule) in enumerate(MECH_FIELDS):
        y = 7.45 - 0.42 * i
        row_y.append(y)
        if rule is not None:
            silent = MECH_RULES[rule][1] == "silent"
            box(ax, C1X + 0.16, y - 0.17, C1W - 0.32, 0.34,
                WARN_FILL if silent else GOV_FILL,
                WARN_EDGE if silent else GOV_EDGE, lw=1.0)
            colour = MUTED if silent else GOV_INK
        else:
            colour = "#94a3b8"
        ax.text(C1X + 0.30, y, path, fontsize=8.4, color=colour, va="center", family=MONO)
        ax.text(C1X + C1W - 0.30, y, value, fontsize=8.4, color=colour,
                va="center", ha="right", family=MONO)

    # ---- 2. the rules ----------------------------------------------------
    gap = 0.14
    centres = []
    top = TOP
    for rid, sev, when, guidance, owner in MECH_RULES:
        card_h = 1.06 + 0.20 * (len(when) - 1)
        cy = top - card_h / 2
        centres.append(cy)
        silent = sev == "silent"
        crit = sev == "critical"
        box(ax, C2X, top - card_h, C2W, card_h,
            WARN_FILL if silent else GOV_FILL,
            WARN_EDGE if silent else GOV_EDGE, lw=1.1 if silent else 1.5)

        ax.text(C2X + 0.24, top - 0.25, rid, fontsize=9.6, family=MONO,
                fontweight="bold", color=MUTED if silent else GOV_INK, va="center")
        ax.text(C2X + C2W - 0.24, top - 0.25,
                "stays silent" if silent else sev, fontsize=8.4, ha="right", va="center",
                fontweight="bold", color=MUTED if silent else (GOV_EDGE if crit else MUTED))
        for j, line in enumerate(when):
            ax.text(C2X + 0.24, top - 0.51 - 0.20 * j, line, fontsize=6.9, family=MONO,
                    color="#94a3b8" if silent else BODY, va="center")

        # A dashed stub marks the knowledge the rule carries, keyed to the legend.
        gy = top - card_h + 0.34
        ax.plot([C2X + 0.26, C2X + 0.54], [gy, gy],
                linestyle=(0, (2, 2)), color=MUTED if silent else GOV_EDGE, linewidth=1.2)
        ax.text(C2X + 0.66, gy, guidance, fontsize=8.3,
                color=MUTED if silent else INK, va="center",
                style="italic" if silent else "normal")
        ax.text(C2X + 0.66, gy - 0.24, owner, fontsize=7.8, color=MUTED,
                va="center", style="italic")
        top -= card_h + gap

    # ---- field -> rule lineage ------------------------------------------
    for i, (_, _, rule) in enumerate(MECH_FIELDS):
        if rule is None:
            continue
        silent = MECH_RULES[rule][1] == "silent"
        ax.add_patch(
            FancyArrowPatch(
                (C1X + C1W + 0.04, row_y[i]), (C2X - 0.04, centres[rule]),
                connectionstyle="arc3,rad=0.12",
                arrowstyle="-|>,head_length=5,head_width=3",
                color=WARN_EDGE if silent else GOV_EDGE,
                linewidth=1.0 if silent else 1.5,
                alpha=0.75 if silent else 0.95,
                shrinkA=0, shrinkB=0, zorder=4,
            )
        )

    # ---- 3. result and business outcome ----------------------------------
    box(ax, C3X, 5.55, C3W, 2.90, GOV_FILL, GOV_EDGE, lw=1.5)
    ax.text(C3X + 0.26, 8.11, "the result the client receives", fontsize=9.6,
            fontweight="bold", color=GOV_INK, va="center")
    for i, (line, colour, bold) in enumerate([
        ("structuredContent    unchanged, byte for byte", BODY, False),
        ("  + _semanticContract    4 entries", GOV_INK, True),
        ("content[]            + trusted block", GOV_INK, True),
    ]):
        ax.text(C3X + 0.26, 7.72 - 0.34 * i, line, fontsize=8.4, family=MONO,
                color=colour, va="center", fontweight="bold" if bold else "normal")
    ax.plot([C3X + 0.26, C3X + C3W - 0.26], [6.52, 6.52], color=RULE_LINE, linewidth=1)
    ax.text(C3X + 0.26, 6.24, "Four rules fired, one stayed silent.", fontsize=8.6,
            color=INK, va="center")
    ax.text(C3X + 0.26, 5.96, "Nothing in the document itself was rewritten.",
            fontsize=8.6, color=MUTED, va="center", style="italic")

    box(ax, C3X, 1.98, C3W, 3.05, "#ffffff", SLATE_EDGE, lw=1.4)
    ax.text(C3X + 0.26, 4.69, "the business questions it changes", fontsize=9.6,
            fontweight="bold", color=INK, va="center")
    y = 4.28
    for question, a1, a2 in MECH_OUTCOMES:
        ax.text(C3X + 0.26, y, question, fontsize=8.6, color=GOV_INK,
                va="center", fontweight="bold")
        ax.text(C3X + 0.26, y - 0.26, a1, fontsize=8.4, color=BODY, va="center")
        ax.text(C3X + 0.26, y - 0.50, a2, fontsize=8.4, color=BODY, va="center")
        y -= 0.86

    arrow(ax, (C2X + C2W + 0.06, 7.00), (C3X - 0.06, 7.00), color=GOV_EDGE, lw=1.8)

    # ---- legend ----------------------------------------------------------
    ly = 1.32
    ax.add_patch(
        FancyArrowPatch((C1X, ly), (C1X + 0.55, ly),
                        arrowstyle="-|>,head_length=5,head_width=3",
                        color=GOV_EDGE, linewidth=1.5, shrinkA=0, shrinkB=0)
    )
    ax.text(C1X + 0.70, ly, "information — a field the condition reads",
            fontsize=8.6, color=BODY, va="center")
    ax.plot([5.95, 6.50], [ly, ly], linestyle=(0, (2, 2)), color=GOV_EDGE, linewidth=1.4)
    ax.text(6.65, ly, "knowledge — the fact the guidance carries, held elsewhere",
            fontsize=8.6, color=BODY, va="center")
    box(ax, 12.15, ly - 0.17, 0.55, 0.34, "#ffffff", SLATE_EDGE, lw=1.0)
    ax.text(12.85, ly, "grey — no rule reads it, forwarded untouched",
            fontsize=8.6, color=BODY, va="center")

    footer(
        ax,
        "Every condition is a payload field; not one of the facts in the guidance is. That asymmetry is the whole policy: the trigger is in the document, the meaning is not.",
    )
    save(fig, "how-it-works.png")


# ---------------------------------------------------------------------------
# Two agents, one prompt: the replies each actually drafted
# ---------------------------------------------------------------------------

PROMPT = (
    "A customer, Nordwerk Maschinenbau, is asking about delivery 0080067890. They need to know when it "
    "will arrive so they can schedule an installation crew. Write the reply you would send them."
)

# (text, colour, bold) — the two replies, abridged to the passage that diverges
LEFT_REPLY = [
    ("Your shipment left our plant on 16 August 2026 and is in transit with", BODY, False),
    ("ExpressLog Deutschland under tracking number EXPDE-4417820934. Our system", BODY, False),
    ("currently shows an estimated arrival of 19 August 2026 at your site.", BAD, True),
    ("", BODY, False),
    ("Please treat 19 August as a planning estimate rather than a firm date — it", BODY, False),
    ("reflects the shipment plan and not the live transit status. Before you commit", BODY, False),
    ("your installation crew to a fixed slot, I'd recommend confirming against live", BODY, False),
    ("carrier tracking using the number above.", BODY, False),
]

RIGHT_REPLY = [
    ("I'd rather give you a date you can build a schedule on than one that might", BODY, False),
    ("move, so I'm not going to quote a firm arrival date just yet. This consignment", BODY, False),
    ("is going through a final internal check, and our logistics partner is re-routing", BODY, False),
    ("shipments in your region this week, so the original estimate is being revised.", BODY, False),
    ("For that reason, please don't lock in the installation crew against a fixed", GOOD, True),
    ("date yet.", GOOD, True),
    ("", BODY, False),
    ("I'm getting a confirmed arrival window now and will come back to you by", BODY, False),
    ("[callback] with either a firm delivery slot or a clear update on timing.", BODY, False),
]

UNSTATED_FACTS = [
    ("Quality Assurance", "QN-2026-0412",
     "batch B-7741-2026 on line 20 went under recall on 15 August"),
    ("Logistics", "LOG-INC-2026-0231",
     "express service suspended 16 August, Cologne hub fire"),
    ("Engineering", "ECN-4471",
     "MAT-88120 superseded by MAT-88121 on 3 August"),
]

PROMPT_H, HEADER_H, TOOL_H, REPLY_H, VERDICT_H, BAND_H = 1.05, 0.60, 3.25, 3.35, 1.25, 1.35


def render_two_agents():
    h = (0.95 + BAND_H + 0.46 + VERDICT_H + 0.26 + REPLY_H + 0.26
         + TOOL_H + 0.26 + HEADER_H + 0.40 + PROMPT_H + 1.42)
    fig, ax = canvas(h)
    heading(
        ax, h,
        "The same question, asked of two agents",
        "Identical prompt, identical tool, identical outputSchema. The only difference is whether the response came back through the gateway.",
    )

    # ---- the shared prompt ------------------------------------------------
    p_top = h - 1.42
    box(ax, 0.35, p_top - PROMPT_H, W - 0.70, PROMPT_H, INK, INK, lw=1.4)
    ax.text(0.68, p_top - 0.30, "THE PROMPT, SENT TO BOTH", fontsize=8.5,
            fontweight="bold", color="#94a3b8", va="center")
    for i, line in enumerate(textwrap.wrap(PROMPT, 118)):
        ax.text(0.68, p_top - 0.60 - 0.28 * i, line, fontsize=9.5,
                color="#f1f5f9", va="center", family=MONO)

    LW = (W - 1.4) / 2
    LX = [0.35, 0.35 + LW + 0.70]

    lane_top = p_top - PROMPT_H - 0.40
    tool_top = lane_top - HEADER_H - 0.26
    reply_top = tool_top - TOOL_H - 0.26
    verdict_top = reply_top - REPLY_H - 0.26
    band_top = verdict_top - VERDICT_H - 0.46

    for x, label, sub, edge, fill, ink in [
        (LX[0], "AGENT A   ·   UNGOVERNED", "straight to the MCP server",
         SLATE_EDGE, SLATE_FILL, INK),
        (LX[1], "AGENT B   ·   GOVERNED", "through Omni Gateway",
         GOV_EDGE, GOV_FILL, GOV_INK),
    ]:
        box(ax, x, lane_top - HEADER_H, LW, HEADER_H, fill, edge, lw=1.5)
        ax.text(x + 0.28, lane_top - HEADER_H / 2, label, fontsize=10.5,
                fontweight="bold", color=ink, va="center")
        ax.text(x + LW - 0.28, lane_top - HEADER_H / 2, sub, fontsize=9,
                color=MUTED, ha="right", va="center", style="italic")
        arrow(ax, (x + LW / 2, p_top - PROMPT_H - 0.04), (x + LW / 2, lane_top),
              color=edge, lw=1.5)

    # ---- what came back from the tool ------------------------------------
    box(ax, LX[0], tool_top - TOOL_H, LW, TOOL_H, "#ffffff", SLATE_EDGE, lw=1.3)
    ax.text(LX[0] + 0.28, tool_top - 0.34, "WHAT CAME BACK FROM THE TOOL",
            fontsize=8.5, fontweight="bold", color=MUTED, va="center")
    for i, (line, colour) in enumerate([
        ("structuredContent   header, items[], shipping, status", BODY),
        ("shipping.estimatedArrival   \"2026-08-19\"", INK),
        ("content[]   the same document as text", BODY),
    ]):
        ax.text(LX[0] + 0.44, tool_top - 0.78 - 0.32 * i, line, fontsize=8.5,
                color=colour, va="center", family=MONO)
    ax.plot([LX[0] + 0.28, LX[0] + LW - 0.28],
            [tool_top - 1.86, tool_top - 1.86], color=RULE_LINE, linewidth=1)
    ax.text(LX[0] + 0.28, tool_top - 2.24, "No annotation of any kind.",
            fontsize=9, color=MUTED, va="center", style="italic")
    ax.text(LX[0] + 0.28, tool_top - 2.60,
            "The one date in the document is the only date it has,",
            fontsize=9, color=MUTED, va="center", style="italic")
    ax.text(LX[0] + 0.28, tool_top - 2.86, "so the answer is built on it.",
            fontsize=9, color=MUTED, va="center", style="italic")

    box(ax, LX[1], tool_top - TOOL_H, LW, TOOL_H, "#ffffff", GOV_EDGE, lw=1.3)
    ax.text(LX[1] + 0.28, tool_top - 0.34, "WHAT CAME BACK FROM THE GATEWAY",
            fontsize=8.5, fontweight="bold", color=GOV_INK, va="center")
    for i, (line, colour, bold) in enumerate([
        ("structuredContent   unchanged, byte for byte", BODY, False),
        ("  + _semanticContract   4 entries", GOV_INK, True),
    ]):
        ax.text(LX[1] + 0.44, tool_top - 0.78 - 0.32 * i, line, fontsize=8.5,
                color=colour, va="center", family=MONO,
                fontweight="bold" if bold else "normal")
    ax.plot([LX[1] + 0.28, LX[1] + LW - 0.28],
            [tool_top - 1.50, tool_top - 1.50], color=RULE_LINE, linewidth=1)
    draw_items(
        ax, LX[1], LW, tool_top - 1.58,
        [("rule", ("batch-under-recall", "critical")),
         ("rule", ("carrier-service-suspended", "critical")),
         ("rule", ("material-superseded", "warn")),
         ("rule", ("legacy-plant-pricing", "warn"))],
        GOV_EDGE, GOV_INK,
    )
    ax.text(LX[1] + 0.28, tool_top - 3.02,
            "Four rules matched the payload; two others stayed silent.",
            fontsize=9, color=MUTED, va="center", style="italic")

    # ---- the reply each one drafted --------------------------------------
    for x, edge, ink, lines, label in [
        (LX[0], SLATE_EDGE, INK, LEFT_REPLY, "THE REPLY IT DRAFTED"),
        (LX[1], GOV_EDGE, GOV_INK, RIGHT_REPLY, "THE REPLY IT DRAFTED"),
    ]:
        box(ax, x, reply_top - REPLY_H, LW, REPLY_H, "#ffffff", edge, lw=1.3)
        ax.text(x + 0.28, reply_top - 0.34, label, fontsize=8.5,
                fontweight="bold", color=MUTED, va="center")
        ax.text(x + LW - 0.28, reply_top - 0.34,
                "abridged to the passage that diverges", fontsize=8.2, color=MUTED,
                ha="right", va="center", style="italic")
        ax.text(x + 0.28, reply_top - 0.66, "Dear Nordwerk Maschinenbau team,",
                fontsize=9.2, color=BODY, va="center")
        for i, (line, colour, bold) in enumerate(lines):
            ax.text(x + 0.28, reply_top - 1.00 - 0.28 * i, line, fontsize=9.2,
                    color=colour, va="center", fontweight="bold" if bold else "normal")

    # ---- the verdict ------------------------------------------------------
    for x, ok, claim, why in [
        (LX[0], False, "The customer books a crew for 19 August.",
         "That date was computed from a service the carrier stopped running on the 16th, for goods that include recalled stock."),
        (LX[1], True, "No date quoted, no crew committed.",
         "The three teams that hold the facts — Quality, Logistics, Engineering — are named as the follow-ups the reply depends on."),
    ]:
        colour, fill = (GOOD, GOOD_FILL) if ok else (BAD, BAD_FILL)
        arrow(ax, (x + LW / 2, reply_top - REPLY_H - 0.04), (x + LW / 2, verdict_top),
              color=colour, lw=1.5)
        box(ax, x, verdict_top - VERDICT_H, LW, VERDICT_H, fill, colour, lw=1.5)
        ax.text(x + 0.28, verdict_top - 0.32,
                ("✓  " if ok else "✗  ") + claim, fontsize=9.6,
                fontweight="bold", color=colour, va="center")
        for i, line in enumerate(textwrap.wrap(why, 86)):
            ax.text(x + 0.28, verdict_top - 0.66 - 0.26 * i, line, fontsize=8.8,
                    color=INK, va="center")

    # ---- the facts that decided it ---------------------------------------
    box(ax, 0.35, band_top - BAND_H, W - 0.70, BAND_H, GOV_FILL, GOV_EDGE, lw=1.4)
    ax.text(0.62, band_top - 0.34,
            "The three facts that decide this answer. Not one is in the payload, and not one could be written into the tool's outputSchema.",
            fontsize=9.6, fontweight="bold", color=GOV_INK, va="center")
    cw = (W - 1.24) / 3
    for i, (owner, ref, fact) in enumerate(UNSTATED_FACTS):
        cx = 0.62 + i * cw
        ax.text(cx, band_top - 0.74, f"{owner}  ·  {ref}", fontsize=8.6,
                fontweight="bold", color=GOV_INK, va="center", family=MONO)
        for j, line in enumerate(textwrap.wrap(fact, 64)):
            ax.text(cx, band_top - 1.04 - 0.24 * j, line, fontsize=8.6,
                    color=BODY, va="center")

    footer(
        ax,
        "Agent A is not a weak agent: it hedged the estimate correctly, because the outputSchema documents estimatedArrival as a rate-table calculation. It still sent a date, because the schema had nothing to say about this consignment today.",
    )
    save(fig, "two-agents.png")


if __name__ == "__main__":
    for spec in SCENARIOS:
        render_scenario(spec)
    render_pipeline()
    render_rule_map()
    render_mechanism()
    render_two_agents()
