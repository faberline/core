const managedAxiosCache = new Map<string, Promise<AxiosInstance>>();

function managedAxios(ca: string, serverName: string): Promise<AxiosInstance> {
  const key = `${serverName}\n${ca}`;
  let instance = managedAxiosCache.get(key);
  if (!instance) {
    instance = (async () => {
      const httpsModule = "node:https";
      const https = (await import(httpsModule)) as any;
      return axios.create({
        httpsAgent: new https.Agent({ ca, servername: serverName, rejectUnauthorized: true }),
      });
    })();
    managedAxiosCache.set(key, instance);
    instance.catch(() => {
      managedAxiosCache.delete(key);
    });
  }
  return instance;
}

