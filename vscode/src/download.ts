import http from "node:http";
import https from "node:https";
import { TLSSocket, connect as connectTls } from "node:tls";

export interface DownloadOptions {
  proxy?: string;
  proxyAuthorization?: string;
  strictSSL?: boolean;
}

interface DownloadResponse {
  status: number;
  location?: string;
  body: Buffer;
}

function responseBuffer(response: http.IncomingMessage): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    response.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > 512 * 1024 * 1024) {
        response.destroy(new Error("Download exceeds the 512 MiB safety limit"));
        return;
      }
      chunks.push(chunk);
    });
    response.on("end", () => resolve(Buffer.concat(chunks)));
    response.on("error", reject);
  });
}

async function proxyTunnel(url: URL, options: DownloadOptions): Promise<TLSSocket> {
  if (!options.proxy) throw new Error("proxyTunnel requires a proxy");
  const proxy = new URL(options.proxy);
  if (proxy.protocol !== "http:" && proxy.protocol !== "https:") {
    throw new Error(`Unsupported VSCode proxy protocol: ${proxy.protocol}`);
  }
  const port = Number(url.port || 443);
  const authority = `${url.hostname}:${port}`;
  const headers: Record<string, string> = { Host: authority };
  if (options.proxyAuthorization) {
    headers["Proxy-Authorization"] = options.proxyAuthorization;
  }
  const connector = proxy.protocol === "https:" ? https : http;
  const socket = await new Promise<import("node:stream").Duplex>((resolve, reject) => {
    const request = connector.request({
      hostname: proxy.hostname,
      port: Number(proxy.port || (proxy.protocol === "https:" ? 443 : 80)),
      method: "CONNECT",
      path: authority,
      headers,
      rejectUnauthorized: options.strictSSL !== false,
    });
    request.on("connect", (response, connected) => {
      if (response.statusCode !== 200) {
        connected.destroy();
        reject(new Error(`VSCode proxy CONNECT failed with HTTP ${response.statusCode}`));
      } else {
        resolve(connected);
      }
    });
    request.on("error", reject);
    request.end();
  });
  return connectTls({
    socket,
    servername: url.hostname,
    rejectUnauthorized: options.strictSSL !== false,
  });
}

async function requestOnce(url: URL, options: DownloadOptions): Promise<DownloadResponse> {
  if (url.protocol !== "https:") throw new Error(`Only HTTPS release URLs are allowed: ${url}`);
  let tunneled: TLSSocket | undefined;
  if (options.proxy) tunneled = await proxyTunnel(url, options);
  return new Promise((resolve, reject) => {
    const requestOptions: https.RequestOptions = {
      hostname: url.hostname,
      port: Number(url.port || 443),
      path: `${url.pathname}${url.search}`,
      method: "GET",
      agent: false,
      rejectUnauthorized: options.strictSSL !== false,
    };
    if (tunneled) requestOptions.createConnection = () => tunneled;
    const request = https.request(requestOptions, (response) => {
      void (async () => {
        try {
          const body = await responseBuffer(response);
          const result: DownloadResponse = {
            status: response.statusCode ?? 0,
            body,
          };
          if (response.headers.location) result.location = response.headers.location;
          resolve(result);
        } catch (error) {
          reject(error);
        }
      })();
    });
    request.on("error", reject);
    request.end();
  });
}

/** Download one official-release URL and follow at most one redirect. */
export async function downloadBuffer(
  address: string,
  options: DownloadOptions = {},
  redirects = 0,
): Promise<Buffer> {
  const url = new URL(address);
  const response = await requestOnce(url, options);
  if ([301, 302, 303, 307, 308].includes(response.status) && response.location) {
    if (redirects >= 1) throw new Error("Release download redirected more than once");
    return downloadBuffer(new URL(response.location, url).toString(), options, redirects + 1);
  }
  if (response.status !== 200) {
    throw new Error(`Download failed with HTTP ${response.status}: ${url}`);
  }
  return response.body;
}
