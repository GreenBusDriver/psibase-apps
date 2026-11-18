/** Merge staged-tx actor events with mcp grant/enforcer audit rows for the UI. */

export type StagedActorEvent = {
    actor: string;
    txid: string;
    eventType: string;
    datetime: string;
};

export type McpHistRow = {
    account: string;
    seq: number;
    kind: string;
    actor: string;
    service: string;
    method: string;
    stagedId: number;
    atSec: number;
    detail: string;
};

export type HistoryKind =
    | "agent-proposal"
    | "owner-approval"
    | "owner-rejection"
    | "enforcer-approval"
    | "enforcer-denial"
    | "permission-grant"
    | "permission-revoke"
    | "other";

export type HistoryItem = {
    key: string;
    at: string;
    kind: HistoryKind;
    label: string;
    detail: string;
    actor: string;
    txid?: string;
};

export function classifyStagedEvent(
    event: StagedActorEvent,
    roles: { owner: string; agent: string; enforcer: string | null },
): HistoryItem | null {
    const { actor, eventType, datetime, txid } = event;
    const short = txid.slice(0, 8);
    if (eventType === "proposed" && actor === roles.agent) {
        return {
            key: `staged:${txid}:proposed`,
            at: datetime,
            kind: "agent-proposal",
            label: "Agent proposal",
            detail: `txid ${short}`,
            actor,
            txid,
        };
    }
    if (eventType === "accepted" && actor === roles.owner) {
        return {
            key: `staged:${txid}:owner-accept`,
            at: datetime,
            kind: "owner-approval",
            label: "Owner approval",
            detail: `txid ${short}`,
            actor,
            txid,
        };
    }
    if (eventType === "rejected" && actor === roles.owner) {
        return {
            key: `staged:${txid}:owner-reject`,
            at: datetime,
            kind: "owner-rejection",
            label: "Owner rejection",
            detail: `txid ${short}`,
            actor,
            txid,
        };
    }
    if (eventType === "accepted" && roles.enforcer && actor === roles.enforcer) {
        return {
            key: `staged:${txid}:enforcer-accept`,
            at: datetime,
            kind: "enforcer-approval",
            label: "Enforcer approval",
            detail: `txid ${short}`,
            actor,
            txid,
        };
    }
    if (eventType === "rejected" && roles.enforcer && actor === roles.enforcer) {
        return {
            key: `staged:${txid}:enforcer-reject`,
            at: datetime,
            kind: "enforcer-denial",
            label: "Enforcer denial",
            detail: `txid ${short}`,
            actor,
            txid,
        };
    }
    return null;
}

export function classifyMcpRow(row: McpHistRow): HistoryItem | null {
    const at = new Date(row.atSec * 1000).toISOString();
    if (row.kind === "granted") {
        return {
            key: `mcp:${row.account}:${row.seq}`,
            at,
            kind: "permission-grant",
            label: "Permission grant",
            detail: row.detail || `${row.service}:${row.method}`,
            actor: row.actor,
        };
    }
    if (row.kind === "revoked") {
        return {
            key: `mcp:${row.account}:${row.seq}`,
            at,
            kind: "permission-revoke",
            label: "Permission revoke",
            detail: row.detail || `${row.service}:${row.method}`,
            actor: row.actor,
        };
    }
    if (row.kind === "enforcerApproved") {
        return {
            key: `mcp:${row.account}:${row.seq}`,
            at,
            kind: "enforcer-approval",
            label: "Enforcer approval",
            detail: row.detail || `staged #${row.stagedId}`,
            actor: row.actor,
        };
    }
    return null;
}

/** Newest first. Drops duplicate enforcer-approval when both sources have it. */
export function mergeHistory(
    staged: HistoryItem[],
    mcp: HistoryItem[],
): HistoryItem[] {
    const out = [...staged, ...mcp];
    const seen = new Set<string>();
    const deduped: HistoryItem[] = [];
    for (const item of out) {
        if (item.kind === "enforcer-approval" && item.txid) {
            const k = `enforcer:${item.txid}`;
            if (seen.has(k)) continue;
            seen.add(k);
        }
        deduped.push(item);
    }
    deduped.sort((a, b) => {
        const ta = Date.parse(a.at);
        const tb = Date.parse(b.at);
        if (!Number.isNaN(ta) && !Number.isNaN(tb) && ta !== tb) return tb - ta;
        if (!Number.isNaN(ta) && Number.isNaN(tb)) return -1;
        if (Number.isNaN(ta) && !Number.isNaN(tb)) return 1;
        return b.at.localeCompare(a.at);
    });
    return deduped;
}

export async function fetchActorHistory(
    rootHost: string,
    actor: string,
    last = 40,
): Promise<StagedActorEvent[]> {
    const url = `${location.protocol}//staged-tx.${rootHost}/graphql`;
    const r = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            query: `{ actorHistory(actor: "${actor}", last: ${last}) {
                edges { node { actor txid eventType datetime } }
            } }`,
        }),
    });
    const j = await r.json();
    return (j?.data?.actorHistory?.edges ?? []).map(
        (e: { node: StagedActorEvent }) => e.node,
    );
}

export function parseMyAgentsPayload(raw: unknown): {
    agents: { account: string; agent: string; enforcer?: string | null }[];
    history: McpHistRow[];
} {
    const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
    if (Array.isArray(parsed)) {
        return { agents: parsed, history: [] };
    }
    if (parsed && typeof parsed === "object") {
        const o = parsed as { agents?: unknown; history?: unknown };
        return {
            agents: Array.isArray(o.agents) ? o.agents : [],
            history: Array.isArray(o.history) ? o.history : [],
        };
    }
    return { agents: [], history: [] };
}

export const OWNER_STORAGE_KEY = "mcp.watchingOwner";
