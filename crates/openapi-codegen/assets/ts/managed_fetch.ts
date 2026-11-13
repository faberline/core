const managedFetchCache = new Map<string, Promise<typeof fetch>>();

function managedFetch(ca: string, serverName: string): Promise<typeof fetch> {
  const key = `${serverName}\n${ca}`;
  let transport = managedFetchCache.get(key);
  if (!transport) {
    transport = (async () => {
      const undiciModule = "undici";
      const { Agent } = (await import(undiciModule)) as any;
      const dispatcher = new Agent({ connect: { ca, servername: serverName } });
      return ((input: any, init?: any) =>
        fetch(input, { ...init, dispatcher })) as unknown as typeof fetch;
    })();
    managedFetchCache.set(key, transport);
    transport.catch(() => {
      managedFetchCache.delete(key);
    });
  }
  return transport;
}

