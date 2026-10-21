import type { RPCSchema } from "electrobun/main";
import type { ConnectionProfile, ConnectionSaveCommand } from "./connections";

type ConnectionRequests = {
  listConnections: { params: undefined; response: ConnectionProfile[] };
  saveConnection: { params: ConnectionSaveCommand; response: ConnectionProfile };
  removeConnection: { params: { readonly id: string }; response: void };
  connectProfile: { params: { readonly id: string }; response: void };
  showLocalFiles: { params: undefined; response: void };
};

export type ConnectionsRPC = {
  bun: RPCSchema<{ requests: ConnectionRequests }>;
  webview: RPCSchema<{ requests: Record<never, never> }>;
};
