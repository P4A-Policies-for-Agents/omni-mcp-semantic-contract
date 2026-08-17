#!/usr/bin/env python3
"""Builds the demo policy configuration from the example contracts.

Both contracts go in inline. The delivery contract was briefly served from a
hash-pinned URL to exercise the remote path against a live gateway; that proved
the fail-closed behaviour but bought nothing operationally, because a mandatory
integrity pin means a republished artifact still needs a policy config edit to
update the pin. See the README section on remote contracts.

Writes demo/policy-config.json, which is gitignored.
"""
import json
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
OUT = pathlib.Path(__file__).resolve().parent / "policy-config.json"

# Contract file stem -> the tools it is expected to govern. The assertion keeps
# the deployed binding honest if someone edits toolMapping in the artifact.
BINDINGS = {
    "erp-sales-order": ["get_sales_order"],
    "erp-delivery": ["get_delivery_document"],
}

DELIMITER = "--- GATEWAY SEMANTIC CONTRACT (trusted) ---"


def inline(stem: str, tools: list[str]) -> dict:
    path = REPO / "example-contracts" / f"{stem}.contract.json"
    contract = json.loads(path.read_text())
    if contract["toolMapping"] != tools:
        sys.exit(
            f"{path.name}: toolMapping is {contract['toolMapping']}, "
            f"expected {tools}. Update BINDINGS or the contract."
        )
    return {
        "contractId": contract["contractId"],
        "format": "json",
        "toolMapping": tools,
        # The policy takes the artifact as a string, not as nested JSON.
        "inline": json.dumps(contract, separators=(",", ":")),
    }


config = {
    "envelope": {
        "delimiter": DELIMITER,
        "sanitizeUpstreamDelimiter": True,
    },
    "contracts": [inline(stem, tools) for stem, tools in BINDINGS.items()],
    "merge": {
        "order": ["json", "url", "markdown", "text"],
        "duplicateRuleIds": "firstWins",
        "globalMaxTokens": 600,
        "onBudgetExceeded": "dropBySeverity",
    },
    "dedupe": {"injectOncePer": "call", "sessionTtlSeconds": 900},
    # A2D frames every tools/call answer as text/event-stream, so without this
    # the policy would govern nothing at all on this upstream.
    "sse": {"mode": "annotate", "streamTimeoutMillis": 60000},
    "warnOnUncoveredTools": True,
}

OUT.write_text(json.dumps(config, indent=2))

for contract in config["contracts"]:
    rules = json.loads(contract["inline"])["rules"]
    print(
        f"  {contract['contractId']:<18} {len(rules)} rules  "
        f"{len(contract['inline']):>5} bytes  -> {', '.join(contract['toolMapping'])}"
    )
print(f"wrote {OUT}")
