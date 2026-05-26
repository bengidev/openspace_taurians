import { describe, it, expect, vi, beforeEach, Mock } from "vitest";
import { invokeStream } from "../lib/stream";

interface MockChannelInstance {
  onmessage: ((message: unknown) => void) | null;
}

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => {
  class MockChannel {
    onmessage: ((message: unknown) => void) | null = null;
  }

  const mockInvoke = vi.fn();

  return {
    invoke: mockInvoke,
    Channel: MockChannel,
  };
});

describe("invokeStream", () => {
  let mockInvoke: Mock;

  beforeEach(async () => {
    vi.clearAllMocks();

    // Get the mocked functions
    const tauriCore = await import("@tauri-apps/api/core");
    mockInvoke = tauriCore.invoke as Mock;
  });

  it("should yield items from the stream", async () => {
    mockInvoke.mockImplementation(
      (_command: string, args: Record<string, unknown>) => {
        const channel = args.onEvent as MockChannelInstance;
        // Simulate sending items
        setTimeout(() => channel.onmessage?.(1), 10);
        setTimeout(() => channel.onmessage?.(2), 20);
        setTimeout(() => channel.onmessage?.(3), 30);
        // Return a promise that resolves after all items are sent
        return new Promise(resolve => setTimeout(resolve, 40));
      }
    );

    const items: number[] = [];
    for await (const item of invokeStream<number>("test_command")) {
      items.push(item);
    }

    expect(items).toEqual([1, 2, 3]);
    expect(mockInvoke).toHaveBeenCalledWith("test_command", {
      onEvent: expect.any(Object),
    });
  });

  it("should pass additional arguments to the command", async () => {
    mockInvoke.mockResolvedValue(undefined);

    const items: unknown[] = [];
    for await (const item of invokeStream("test_command", {
      param1: "value1",
      param2: 42,
    })) {
      items.push(item);
    }

    expect(mockInvoke).toHaveBeenCalledWith("test_command", {
      param1: "value1",
      param2: 42,
      onEvent: expect.any(Object),
    });
  });

  it("should support custom channel parameter name", async () => {
    mockInvoke.mockResolvedValue(undefined);

    const items: unknown[] = [];
    for await (const item of invokeStream(
      "test_command",
      {},
      { channelParam: "customChannel" }
    )) {
      items.push(item);
    }

    expect(mockInvoke).toHaveBeenCalledWith("test_command", {
      customChannel: expect.any(Object),
    });
  });

  it("should handle empty streams", async () => {
    mockInvoke.mockResolvedValue(undefined);

    const items: unknown[] = [];
    for await (const item of invokeStream("test_command")) {
      items.push(item);
    }

    expect(items).toEqual([]);
  });

  it("should propagate errors from the command", async () => {
    const testError = new Error("Command failed");
    mockInvoke.mockRejectedValue(testError);

    const items: unknown[] = [];
    await expect(async () => {
      for await (const item of invokeStream("test_command")) {
        items.push(item);
      }
    }).rejects.toThrow("Command failed");
  });

  it("should handle items arriving before iteration starts", async () => {
    mockInvoke.mockImplementation(
      (_command: string, args: Record<string, unknown>) => {
        const channel = args.onEvent as MockChannelInstance;
        // Send items immediately
        channel.onmessage?.("a");
        channel.onmessage?.("b");
        channel.onmessage?.("c");
        return Promise.resolve();
      }
    );

    const items: string[] = [];
    for await (const item of invokeStream<string>("test_command")) {
      items.push(item);
    }

    expect(items).toEqual(["a", "b", "c"]);
  });

  it("should handle items arriving after iteration starts", async () => {
    let resolveInvoke: () => void;
    const invokePromise = new Promise<void>((resolve) => {
      resolveInvoke = resolve;
    });

    mockInvoke.mockImplementation(
      (_command: string, args: Record<string, unknown>) => {
        const channel = args.onEvent as MockChannelInstance;
        // Send items with delays
        setTimeout(() => channel.onmessage?.(10), 10);
        setTimeout(() => channel.onmessage?.(20), 20);
        setTimeout(() => {
          channel.onmessage?.(30);
          resolveInvoke();
        }, 30);
        return invokePromise;
      }
    );

    const items: number[] = [];
    for await (const item of invokeStream<number>("test_command")) {
      items.push(item);
    }

    expect(items).toEqual([10, 20, 30]);
  });

  it("should handle mixed timing of items", async () => {
    mockInvoke.mockImplementation(
      (_command: string, args: Record<string, unknown>) => {
        const channel = args.onEvent as MockChannelInstance;
        // Some immediate, some delayed
        channel.onmessage?.("immediate1");
        setTimeout(() => channel.onmessage?.("delayed1"), 10);
        channel.onmessage?.("immediate2");
        setTimeout(() => channel.onmessage?.("delayed2"), 20);
        // Return a promise that resolves after all items are sent
        return new Promise(resolve => setTimeout(resolve, 30));
      }
    );

    const items: string[] = [];
    for await (const item of invokeStream<string>("test_command")) {
      items.push(item);
    }

    expect(items).toEqual(["immediate1", "immediate2", "delayed1", "delayed2"]);
  });

  it("should convert non-Error rejections to Error objects", async () => {
    mockInvoke.mockRejectedValue("string error");

    await expect(async () => {
      for await (const item of invokeStream("test_command")) {
        // consume
        void item;
      }
    }).rejects.toThrow("string error");
  });
});
