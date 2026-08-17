#!/usr/bin/env bash
#
# Prints which rules fire for each demo scenario through the governed gateway.
# Run it against the raw A2D URL instead to see the ungoverned baseline:
#
#   GATEWAY_URL="https://www.a2d-ai.com/api/platform/$A2D_SERVER_ID/mcp" \
#   A2D_API_KEY=... ./demo/verify.sh
set -euo pipefail

GATEWAY_URL="${GATEWAY_URL:-https://omni-gateway-shared-space-zovwbn.5sc6y6-1.usa-e2.cloudhub.io/erp_sales_order_mcp/mcp}"
# Only needed when pointing straight at A2D; the gateway injects its own key.
A2D_API_KEY="${A2D_API_KEY:-}"

call() {
  local tool="$1" arg_name="$2" arg_value="$3"
  curl -sS -X POST "$GATEWAY_URL" \
    ${A2D_API_KEY:+-H "Authorization: Bearer $A2D_API_KEY"} \
    -H "content-type: application/json" \
    -H "accept: application/json, text/event-stream" \
    --data-binary "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$tool\",\"arguments\":{\"$arg_name\":\"$arg_value\"}}}"
}

report() {
  python3 -c "
import json, sys

raw = sys.stdin.read()
frames = [l[6:] for l in raw.splitlines() if l.startswith('data: ')]
body = json.loads(frames[0] if frames else raw)

if 'error' in body:
    print('    error:', body['error'].get('message', '')[:120])
    raise SystemExit

result = body['result']
fired = result.get('structuredContent', {}).get('_semanticContract', [])
print(f'    {len(fired)} rule(s), {len(result.get(\"content\", []))} content element(s)')
for entry in fired:
    print('      -', entry.split(':')[0])
"
}

echo "  get_delivery_document 0080067890  (looks healthy on paper)"
call get_delivery_document deliveryId 0080067890 | report
echo "  get_delivery_document 0080055512  (export controlled, customer in dispute)"
call get_delivery_document deliveryId 0080055512 | report
echo "  get_delivery_document 0080012345  (clean, expect silence)"
call get_delivery_document deliveryId 0080012345 | report
echo "  get_sales_order       0000004711  (the original scenario)"
call get_sales_order salesOrderId 0000004711 | report
