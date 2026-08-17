# Example contracts

Worked examples for the [MCP Semantic Contract](../policies/mcp-semantic-contract/mcp-semantic-contract-flex/README.md)
policy. Nothing here is policy code and nothing is loaded at build time: these
are reference artifacts for the demo, and the material the tests are pinned
against.

Two scenarios, each a matched pair of a tool `outputSchema` and a contract. The
pairing is the point. The schema carries everything true of a field on every
call; the contract carries only what the schema structurally cannot.

| File | What it is |
|---|---|
| `semantic-contract-v1.schema.json` | JSON Schema for the contract format itself. Validate a contract against this before deploying it. |
| `erp-sales-order.outputschema.json` | Tool output schema: SAP SD sales order vocabulary |
| `erp-sales-order.contract.json` | 5 rules, each a conclusion two or more fields imply together |
| `erp-sales-order.response.json` | A recorded `tools/call` result, used as a test fixture |
| `erp-delivery.outputschema.json` | Tool output schema: SAP SD outbound delivery vocabulary |
| `erp-delivery.contract.json` | 6 rules, each a fact held in a system the ERP does not read |
| `erp-delivery.mocks.json` | The three demo delivery documents, keyed by `deliveryId` |

## The two contracts demonstrate different things

**`erp-sales-order`** is the conditional case. Every rule is a cross-field
conclusion — a credit block whose exposure exceeds the limit, a confirmed date
that was never rescheduled. These rules are genuinely conditional, and each one
has a payload that silences it.

They are also the weaker argument, and it is worth being honest about why.
Measured against a capable model reading `erp-sales-order.outputschema.json`,
most of them turned out to be redundant: told once what `deliveredQuantity`
counts, the model reaches the conclusion unaided. A rule of this kind buys
salience, not knowledge.

**`erp-delivery`** is the case a good schema cannot dissolve. Every rule is
triggered by a payload value but its *guidance* states something that is not in
the payload at all and could not have been written when the tool shipped:

| Rule | Fires on | Fact comes from |
|---|---|---|
| `batch-under-recall` | a batch number | Quality Assurance, changes daily |
| `carrier-service-suspended` | carrier and service level | a Logistics incident report |
| `export-licence-missing` | an ECCN with no licence | trade compliance standard TC-114 |
| `customer-communication-hold` | the ship-to party | an open legal matter |
| `material-superseded` | a material number | an engineering change order |
| `legacy-plant-pricing` | plant and creation date | a system migration cutover date |

Note that no rule's owner is the team that owns the API. That is the whole
argument for putting this at the gateway rather than in the tool.

## Using them

Validate before deploying:

```bash
python3 -c "import json;json.load(open('example-contracts/erp-delivery.contract.json'))"
```

Seed the mock server and deploy both contracts to the gateway:

```bash
A2D_API_KEY=... python3 demo/seed-a2d.py
./demo/deploy.sh
```

`demo/build-policy-config.py` reads the `.contract.json` files directly, so
editing a rule here and re-running the deploy is the whole change cycle. The
`.outputschema.json` files are pushed to the mock server by `demo/seed-a2d.py`.

The grammar for the `when` attribute, the patterns worth knowing and the
precedence trap are documented in
[the policy README](../policies/mcp-semantic-contract/mcp-semantic-contract-flex/README.md#writing-rules-the-when-expression).
