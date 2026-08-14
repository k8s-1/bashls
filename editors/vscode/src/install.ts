import * as vscode from "vscode";
import * as https from "node:https";
import * as fs from "node:fs";
import * as fsp from "node:fs/promises";
import * as path from "node:path";
import * as os from "node:os";
import * as crypto from "node:crypto";
import { execFile } from "node:child_process";

const REPO = "k8s-1/bashls";
const USER_AGENT = "bashls-vscode-extension";

interface GhAsset {
  name: string;
  browser_download_url: string;
}

interface GhRelease {
  tag_name: string;
  assets: GhAsset[];
}

export function getTargetTriple(): string | undefined {
  const platform = os.platform();
  const arch = os.arch();
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-musl";
  if (platform === "linux" && arch === "arm64") return "aarch64-unknown-linux-musl";
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin";
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin";
  return undefined;
}

function httpGet(url: string): Promise<{ status: number; headers: Record<string, string | string[] | undefined>; body: Buffer }> {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": USER_AGENT } }, (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () =>
          resolve({
            status: res.statusCode ?? 0,
            headers: res.headers,
            body: Buffer.concat(chunks),
          }),
        );
      })
      .on("error", reject);
  });
}

async function httpGetFollowingRedirects(url: string): Promise<Buffer> {
  const res = await httpGet(url);
  if (res.status >= 300 && res.status < 400 && res.headers.location) {
    return httpGetFollowingRedirects(String(res.headers.location));
  }
  if (res.status !== 200) {
    throw new Error(`Request to ${url} failed: HTTP ${res.status}`);
  }
  return res.body;
}

async function downloadFile(url: string, dest: string): Promise<void> {
  const body = await httpGetFollowingRedirects(url);
  await fsp.writeFile(dest, body);
}

async function sha256File(filePath: string): Promise<string> {
  const hash = crypto.createHash("sha256");
  hash.update(await fsp.readFile(filePath));
  return hash.digest("hex");
}

function run(command: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    execFile(command, args, { cwd }, (error) => (error ? reject(error) : resolve()));
  });
}

async function findLatestCliRelease(): Promise<GhRelease> {
  const body = await httpGetFollowingRedirects(
    `https://api.github.com/repos/${REPO}/releases`,
  );
  const releases = JSON.parse(body.toString("utf8")) as GhRelease[];
  const release = releases.find((r) => /^v\d+\.\d+\.\d+$/.test(r.tag_name));
  if (!release) {
    throw new Error("Could not find a bashls CLI release on GitHub.");
  }
  return release;
}

export async function installBashls(
  context: vscode.ExtensionContext,
  progress: vscode.Progress<{ message?: string }>,
): Promise<string> {
  const triple = getTargetTriple();
  if (!triple) {
    throw new Error(
      `No prebuilt bashls binary is published for ${os.platform()}/${os.arch()}.`,
    );
  }

  progress.report({ message: "Checking latest release..." });
  const release = await findLatestCliRelease();

  const assetName = `bashls-${triple}.tar.gz`;
  const asset = release.assets.find((a) => a.name === assetName);
  const checksumsAsset = release.assets.find((a) => a.name === "checksums.txt");
  if (!asset || !checksumsAsset) {
    throw new Error(`Release ${release.tag_name} is missing ${assetName} or checksums.txt.`);
  }

  const installDir = path.join(
    context.globalStorageUri.fsPath,
    "bashls",
    release.tag_name,
  );
  await fsp.mkdir(installDir, { recursive: true });

  const tarPath = path.join(installDir, assetName);
  const binaryPath = path.join(installDir, "bashls");

  if (fs.existsSync(binaryPath)) {
    return binaryPath;
  }

  progress.report({ message: `Downloading ${assetName}...` });
  await downloadFile(asset.browser_download_url, tarPath);

  progress.report({ message: "Verifying checksum..." });
  const checksumsText = (
    await httpGetFollowingRedirects(checksumsAsset.browser_download_url)
  ).toString("utf8");
  const line = checksumsText.split("\n").find((l) => l.includes(assetName));
  const expected = line?.trim().split(/\s+/)[0];
  if (!expected) {
    throw new Error(`No checksum entry for ${assetName} in checksums.txt.`);
  }
  const actual = await sha256File(tarPath);
  if (actual !== expected) {
    throw new Error(`Checksum mismatch for ${assetName}: expected ${expected}, got ${actual}.`);
  }

  progress.report({ message: "Extracting..." });
  await run("tar", ["-xzf", assetName], installDir);
  await fsp.chmod(binaryPath, 0o755);
  await fsp.rm(tarPath, { force: true });

  return binaryPath;
}
