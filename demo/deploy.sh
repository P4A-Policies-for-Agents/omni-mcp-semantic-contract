#!/usr/bin/env bash
#
# Deploys the demo semantic contract configuration to a Flex Gateway API
# instance, then probes every demo scenario through the gateway.
#
#   ./demo/deploy.sh
#
# Requires anypoint-cli-v4 already authenticated. Everything below can be
# overridden by environment variable; the defaults are the demo instance.
set -euo pipefail

API_ID="${API_ID:-21100028}"
ENVIRONMENT="${ENVIRONMENT:-Sandbox}"
GROUP_ID="${GROUP_ID:-82a0453b-22e6-430d-bbf4-35b989d043dc}"
POLICY_ASSET="${POLICY_ASSET:-mcp-semantic-contract}"
POLICY_VERSION="${POLICY_VERSION:-1.1.1}"
GATEWAY_URL="${GATEWAY_URL:-https://omni-gateway-shared-space-zovwbn.5sc6y6-1.usa-e2.cloudhub.io/erp_sales_order_mcp/mcp}"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="$HERE/policy-config.json"

echo "==> Building policy configuration from example-contracts/"
python3 "$HERE/build-policy-config.py"

echo
echo "==> Looking for an applied $POLICY_ASSET policy on API $API_ID"
POLICY_ID="$(
  anypoint-cli-v4 api-mgr:policy:list "$API_ID" --environment "$ENVIRONMENT" -o json \
  | python3 -c "
import json, sys
applied = json.load(sys.stdin)
match = [p for p in applied if p.get('Asset ID') == '$POLICY_ASSET']
print(match[0]['ID'] if match else '')
"
)"

if [ -n "$POLICY_ID" ]; then
  echo "    found policy $POLICY_ID, updating its configuration"
  anypoint-cli-v4 api-mgr:policy:edit "$API_ID" "$POLICY_ID" \
    --environment "$ENVIRONMENT" --configFile "$CONFIG" -o json
else
  echo "    none applied, applying $POLICY_ASSET:$POLICY_VERSION"
  anypoint-cli-v4 api-mgr:policy:apply "$API_ID" "$POLICY_ASSET" \
    --policyVersion "$POLICY_VERSION" --groupId "$GROUP_ID" \
    --environment "$ENVIRONMENT" --configFile "$CONFIG" -o json
fi

echo
echo "==> Waiting for the gateway to pick up the configuration"
sleep 45

echo
echo "==> Probing the demo scenarios through the gateway"
"$HERE/verify.sh"
