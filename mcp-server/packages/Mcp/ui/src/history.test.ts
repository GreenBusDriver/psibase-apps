import { describe, expect, it } from "vitest";

import {
    classifyMcpRow,
    classifyStagedEvent,
    mergeHistory,
    parseMyAgentsPayload,
} from "./lib/history";

const roles = {
    owner: "alicealice",
    agent: "aagent1234",
    enforcer: "aenforcer",
};

describe("history classification", () => {
    it("labels agent propose / owner accept-reject / enforcer accept-reject", () => {
        expect(
            classifyStagedEvent(
                {
                    actor: "aagent1234",
                    txid: "AA",
                    eventType: "proposed",
                    datetime: "2026-11-10T19:52:02+00:00",
                },
                roles,
            )?.kind,
        ).toBe("agent-proposal");
        expect(
            classifyStagedEvent(
                {
                    actor: "alicealice",
                    txid: "AA",
                    eventType: "accepted",
                    datetime: "2026-11-10T19:54:50+00:00",
                },
                roles,
            )?.kind,
        ).toBe("owner-approval");
        expect(
            classifyStagedEvent(
                {
                    actor: "alicealice",
                    txid: "BB",
                    eventType: "rejected",
                    datetime: "2026-11-10T19:58:03+00:00",
                },
                roles,
            )?.kind,
        ).toBe("owner-rejection");
        expect(
            classifyStagedEvent(
                {
                    actor: "aenforcer",
                    txid: "CC",
                    eventType: "accepted",
                    datetime: "2026-11-10T20:00:00+00:00",
                },
                roles,
            )?.kind,
        ).toBe("enforcer-approval");
        expect(
            classifyStagedEvent(
                {
                    actor: "aenforcer",
                    txid: "DD",
                    eventType: "rejected",
                    datetime: "2026-11-10T20:01:00+00:00",
                },
                roles,
            )?.kind,
        ).toBe("enforcer-denial");
    });

    it("ignores agent accept and unrelated actors", () => {
        expect(
            classifyStagedEvent(
                {
                    actor: "aagent1234",
                    txid: "AA",
                    eventType: "accepted",
                    datetime: "2026-11-10T19:52:02+00:00",
                },
                roles,
            ),
        ).toBeNull();
        expect(
            classifyStagedEvent(
                {
                    actor: "myprod",
                    txid: "EE",
                    eventType: "rejected",
                    datetime: "2026-11-10T19:58:03+00:00",
                },
                roles,
            ),
        ).toBeNull();
    });

    it("labels mcp grant / revoke / enforcerApproved rows", () => {
        expect(
            classifyMcpRow({
                account: "alicealice",
                seq: 1,
                kind: "granted",
                actor: "alicealice",
                service: "tokens",
                method: "credit",
                stagedId: 3,
                atSec: 1_762_800_000,
                detail: "tokens:credit",
            })?.kind,
        ).toBe("permission-grant");
        expect(
            classifyMcpRow({
                account: "alicealice",
                seq: 2,
                kind: "revoked",
                actor: "alicealice",
                service: "tokens",
                method: "credit",
                stagedId: 0,
                atSec: 1_762_800_100,
                detail: "tokens:credit",
            })?.kind,
        ).toBe("permission-revoke");
        expect(
            classifyMcpRow({
                account: "alicealice",
                seq: 3,
                kind: "enforcerApproved",
                actor: "aenforcer",
                service: "",
                method: "",
                stagedId: 5,
                atSec: 1_762_800_200,
                detail: "staged #5",
            })?.kind,
        ).toBe("enforcer-approval");
    });

    it("merges newest-first and parses my-agents payload shapes", () => {
        const merged = mergeHistory(
            [
                {
                    key: "a",
                    at: "2026-11-10T19:00:00.000Z",
                    kind: "agent-proposal",
                    label: "Agent proposal",
                    detail: "old",
                    actor: "aagent1234",
                },
                {
                    key: "b",
                    at: "2026-11-10T20:00:00.000Z",
                    kind: "owner-approval",
                    label: "Owner approval",
                    detail: "new",
                    actor: "alicealice",
                },
            ],
            [],
        );
        expect(merged.map((i) => i.key)).toEqual(["b", "a"]);

        expect(parseMyAgentsPayload("[]")).toEqual({
            agents: [],
            history: [],
        });
        expect(
            parseMyAgentsPayload({
                agents: [{ account: "o", agent: "a", enforcer: "e" }],
                history: [],
            }).agents[0].agent,
        ).toBe("a");
    });
});
