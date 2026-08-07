import type { ContractSelection, HostInfo, ProtocolResponse } from "./types";

export interface PluroraTransport {
  invoke(method: string, params: unknown): Promise<unknown>;
  invokeWithContract?(
    method: string,
    params: unknown,
    contract: ContractSelection,
  ): Promise<unknown>;
  invokeStream(method: string, params: unknown): AsyncIterable<unknown>;
  close?(): Promise<void>;
}

export class PluroraClient {
  private selectedContract?: ContractSelection;

  constructor(public transport: PluroraTransport) {}

  async invoke(method: string, params: unknown): Promise<unknown> {
    if (!this.selectedContract) return this.transport.invoke(method, params);
    if (!this.transport.invokeWithContract) {
      throw new Error("Plurora transport does not support explicit contract selection");
    }
    return this.transport.invokeWithContract(method, params, this.selectedContract);
  }

  async negotiateHost(selection: ContractSelection): Promise<HostInfo> {
    if (!this.transport.invokeWithContract) {
      throw new Error("Plurora transport does not support explicit contract selection");
    }
    const info = await this.transport.invokeWithContract("host.info", {}, selection) as HostInfo;
    this.selectedContract = selection;
    return info;
  }

  clearContractSelection(): void {
    this.selectedContract = undefined;
  }


}

export function fromHttpRpc(url: string): PluroraClient {
  let nextId = 1;
  const transport: PluroraTransport = {
    async invoke(method: string, params: unknown): Promise<unknown> {
      const response = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: String(nextId++), method, params }),
      });
      if (!response.ok) {
        throw new Error(`Plurora RPC ${method} failed with HTTP ${response.status}`);
      }
      const envelope = (await response.json()) as ProtocolResponse;
      if (envelope.error !== undefined) {
        throw new Error(`Plurora RPC ${method} failed: ${JSON.stringify(envelope.error)}`);
      }
      return envelope.result;
    },
    async invokeWithContract(
      method: string,
      params: unknown,
      contract: ContractSelection,
    ): Promise<unknown> {
      const response = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: String(nextId++), method, params, contract }),
      });
      if (!response.ok) {
        throw new Error(`Plurora RPC ${method} failed with HTTP ${response.status}`);
      }
      const envelope = (await response.json()) as ProtocolResponse;
      if (envelope.error !== undefined) {
        throw new Error(`Plurora RPC ${method} failed: ${JSON.stringify(envelope.error)}`);
      }
      return envelope.result;
    },
    async *invokeStream(method: string, params: unknown): AsyncIterable<unknown> {
      yield await this.invoke(method, params);
    },

  };
  return new PluroraClient(transport);
}

export function fromStdio(stream: NodeJS.ReadWriteStream): PluroraClient {
  let nextId = 1;
  let buffer = "";
  const pending = new Map<string, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();

  stream.on("data", (chunk: Buffer | string) => {
    buffer += chunk.toString();
    for (;;) {
      const newline = buffer.indexOf("\n");
      if (newline < 0) break;
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (line.length === 0) continue;
      const message = JSON.parse(line) as ProtocolResponse;
      if (message.id === undefined) continue;
      const responseId = String(message.id);
      const waiter = pending.get(responseId);
      if (!waiter) continue;
      pending.delete(responseId);
      if (message.error !== undefined) {
        waiter.reject(new Error(JSON.stringify(message.error)));
      } else {
        waiter.resolve(message.result);
      }
    }
  });

  const transport: PluroraTransport = {
    invoke(method: string, params: unknown): Promise<unknown> {
      const id = String(nextId++);
      const request = { jsonrpc: "2.0", id, method, params };
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        stream.write(`${JSON.stringify(request)}\n`);
      });
    },
    invokeWithContract(
      method: string,
      params: unknown,
      contract: ContractSelection,
    ): Promise<unknown> {
      const id = String(nextId++);
      const request = { jsonrpc: "2.0", id, method, params, contract };
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        stream.write(`${JSON.stringify(request)}\n`);
      });
    },
    async *invokeStream(method: string, params: unknown): AsyncIterable<unknown> {
      yield await this.invoke(method, params);
    },

    async close(): Promise<void> {
      stream.end();
    },
  };
  return new PluroraClient(transport);
}
