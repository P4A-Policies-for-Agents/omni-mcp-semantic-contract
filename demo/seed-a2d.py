#!/usr/bin/env python3
"""Creates or updates the demo tools on the A2D mock MCP server.

A2D is driven over MCP itself: its control plane is an MCP server whose tools
manage mock servers. Everything here is a `tools/call` against that control
plane.

    A2D_API_KEY=... python3 demo/seed-a2d.py

Seeds `get_delivery_document` with the output schema from
example-contracts/erp-delivery.outputschema.json and one mock scenario per
document in example-contracts/erp-delivery.mocks.json. Safe to re-run: an
existing tool is updated in place rather than duplicated.
"""
import json
import os
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
CONTROL_PLANE = "https://www.a2d-ai.com/api/platform-mcp/mcp"
SERVER_ID = os.environ.get("A2D_SERVER_ID", "b50f9d38-87f0-4727-9bfd-f5a4e9b5b395")
API_KEY = os.environ.get("A2D_API_KEY")

TOOL_NAME = "get_delivery_document"
TOOL_DESCRIPTION = (
    "Returns an outbound delivery document from the ERP system of record. "
    "Every field is documented in the output schema; read those descriptions "
    "before interpreting the result. The document describes the delivery only, "
    "not the state of the world it is moving through."
)
INPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "deliveryId": {
            "type": "string",
            "description": "SAP outbound delivery number, zero-padded to 10 characters.",
        }
    },
    "required": ["deliveryId"],
}
SCENARIO_NAMES = {
    "0080067890": "healthy on paper: recalled batch on a suspended express service",
    "0080012345": "clean delivery, nothing to say",
    "0080055512": "export controlled without a licence, customer in dispute",
}

if not API_KEY:
    sys.exit("A2D_API_KEY is not set")


def call(name: str, arguments: dict) -> str:
    """Invokes one control-plane tool and returns its text result.

    A2D answers with SSE framing and rejects an Accept header that does not
    offer text/event-stream, so both media types have to be advertised.
    """
    body = json.dumps({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    })
    raw = subprocess.run(
        ["curl", "-sS", "-X", "POST", CONTROL_PLANE,
         "-H", f"Authorization: Bearer {API_KEY}",
         "-H", "content-type: application/json",
         "-H", "accept: application/json, text/event-stream",
         "--data-binary", body],
        capture_output=True, text=True, check=True,
    ).stdout
    frames = [line[6:] for line in raw.splitlines() if line.startswith("data: ")]
    payload = json.loads(frames[0] if frames else raw)
    if "error" in payload:
        sys.exit(f"{name} failed: {json.dumps(payload['error'])[:400]}")
    return payload["result"]["content"][0]["text"]


def existing_tool_id(name: str) -> str | None:
    server = json.loads(call("get_mcp_server", {"id": SERVER_ID}))
    server = server.get("server", server)
    for tool in server.get("mcp_tools") or []:
        if tool.get("name") == name:
            return tool.get("id")
    return None


mocks = json.loads((REPO / "example-contracts/erp-delivery.mocks.json").read_text())
output_schema = json.loads(
    (REPO / "example-contracts/erp-delivery.outputschema.json").read_text()
)

# A2D matches scenarios on a top-level input field. `response` must be a JSON
# string, and every scenario needs one: once a tool declares an output schema,
# a scenario returning no structured content fails output validation.
scenarios = [
    {
        "name": SCENARIO_NAMES.get(delivery_id, delivery_id),
        "condition": {"field": "deliveryId", "operator": "===", "value": delivery_id},
        "input": {"deliveryId": delivery_id},
        "response": json.dumps(document),
        "responseType": "json",
    }
    for delivery_id, document in mocks.items()
]

payload = {
    "name": TOOL_NAME,
    "description": TOOL_DESCRIPTION,
    "input_schema": INPUT_SCHEMA,
    "output_schema": output_schema,
    "mock_scenarios": scenarios,
    "enabled": True,
}

tool_id = existing_tool_id(TOOL_NAME)
if tool_id:
    # update_mcp_tool replaces the whole scenario array, so this is a full
    # rewrite rather than a merge.
    print(f"updating existing tool {tool_id}")
    call("update_mcp_tool", {"id": tool_id, **payload})
else:
    print("creating tool")
    call("add_mcp_tool", {"server_id": SERVER_ID, **payload})

print(f"{TOOL_NAME}: {len(scenarios)} scenario(s) -> {', '.join(mocks)}")
print(f"mock server: https://www.a2d-ai.com/api/platform/{SERVER_ID}/mcp")
