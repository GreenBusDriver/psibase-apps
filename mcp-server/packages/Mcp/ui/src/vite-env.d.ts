/// <reference types="vite/client" />

declare module "@psibase/common-lib" {
    export function getSupervisor(): {
        onLoaded(): Promise<void>;
        functionCall(
            args: {
                service: string;
                plugin?: string;
                intf: string;
                method: string;
                params: unknown[];
            },
            opts?: { enabled?: boolean; returnPath?: string },
        ): Promise<unknown>;
    };
}
