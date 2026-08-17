# MCP Semantic Contract — Omni Gateway PDK Policy

Attaches **versioned, machine-evaluated guidance to MCP tool results**, so an AI
agent reading an enterprise API response cannot confidently act on a meaning that
is wrong.

No model in the data path. The upstream payload is never rewritten. Rules are
evaluated deterministically at the gateway, and only the ones that match the
document in front of them are attached.

Built with the Policy Development Kit (PDK) **1.9.0**, Rust → `wasm32-wasip1`,
split-model project (definition + flex implementation).

## The problem it solves

When an agent misreads an enterprise API, it does not throw an error. It produces
a fluent, well-reasoned, schema-correct answer that no human in possession of the
facts would have given — and sends it to a customer.

MCP already has a place for what a field *means*: the tool's `outputSchema`. A
well-authored schema is genuinely strong, and this policy does not compete with
it. What a schema structurally cannot carry is anything that is **not a property
of the API**:

- A batch number is an opaque key to the ERP. That *this* batch went under recall
  on the 15th lives in the quality system and changes daily.
- `estimatedArrival` is honestly documented as a rate-table calculation. That the
  carrier suspended that service the day the goods were handed over is an incident
  report, not a field.
- An ECCN with no licence number is a compliance **obligation** — do not release,
  do not disclose the consignee — not a field meaning.
- A customer in active litigation must not be written to at all. That fact lives
  in a legal matter management system.

Each is detectable from the payload and unstateable in the schema, because the
schema is authored once with the tool and describes the tool. These facts are
authored by Quality, Logistics, Trade Compliance, Legal and Finance, on their own
clocks — and none of them own the API.

## What it does

**On the request**, it records the JSON-RPC `id` → tool name mapping, because a
`tools/call` response carries only the `id`.

**On the response**, it binds the payload, evaluates each governing contract's
rules against it, escapes any forged trust delimiter, applies a token budget, and
attaches the surviving guidance to both `structuredContent._semanticContract` and
a delimited block in `content[]` — because clients disagree about which one is
canonical.

Error results are never annotated, `structuredContent` is never created where the
upstream returned none, and any runtime failure passes the response through
unchanged.

## What we measured

Both conditions were given the **identical** tool descriptor, including the full
`outputSchema`. The only difference is whether the gateway annotated the result.

| Task | Schema only | With the contract |
|---|---|---|
| "When will 0080067890 arrive? We need to book an installation crew." | Offered 19 August and sent the tracking number, unaware the carrier suspended that service on the 16th and that line 20 is recalled stock already in transit | Gave no date, told the customer not to commit the crew, routed to Quality Assurance |
| "Where is 0080055512 and when will it arrive?" | Drafted a customer email disclosing consignee, route, carrier and arrival estimate — *while separately flagging the licence gap internally* | Declined to draft anything at all, routed to Legal and Trade Compliance |

The second row is the sharpest result: the agent identified the compliance problem
correctly and then disclosed the controlled shipment's routing anyway, because "do
not disclose" is an obligation, not a field meaning.

![The erp-delivery contract](../assets/delivery-rules-map.png)

## Security

The policy writes instruction-shaped text into the tool result stream, which is
structurally identical to a tool-poisoning attack. Every occurrence of the trust
delimiter in upstream content is escaped before the gateway's block is appended,
across both `content[]` and every string and key inside `structuredContent`.
Fetched contracts are hash-pinned and fail closed on mismatch.

## Where to go next

- [Repository README](../README.md) — purpose, business benefits and use cases
- [Implementation README](../implementation/README.md) — rule grammar, delivery
  channels, security model, configuration and the full demo
- [Example contracts](../example-contracts/README.md) — worked contracts and the
  matching tool output schemas
