import * as vscode from "vscode";
import * as fs from "node:fs";
import { execFile } from "node:child_process";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import { installBashls } from "./install";

let client: LanguageClient | undefined;

const INSTALL_DOCS_URL = "https://github.com/k8s-1/bashls#installation";
const INSTALLED_PATH_KEY = "bashls.installedPath";

function isBashlsAvailable(command: string): Promise<boolean> {
  return new Promise((resolve) => {
    execFile(command, ["--version"], (error) => {
      const notFound =
        error !== null && (error as NodeJS.ErrnoException).code === "ENOENT";
      resolve(!notFound);
    });
  });
}

function buildInitializationOptions(): Record<string, unknown> {
  const cfg = vscode.workspace.getConfiguration("bashls");
  return {
    bashIde: {
      shellcheckPath: cfg.get("shellcheckPath"),
      shellcheckArguments: cfg.get("shellcheckArguments"),
      shellcheckExternalSources: cfg.get("shellcheckExternalSources"),
      shfmt: { path: cfg.get("shfmt.path") },
      globPattern: cfg.get("globPattern"),
      backgroundAnalysisMaxFiles: cfg.get("backgroundAnalysisMaxFiles"),
      includeAllWorkspaceSymbols: cfg.get("includeAllWorkspaceSymbols"),
      enableSourceErrorDiagnostics: cfg.get("enableSourceErrorDiagnostics"),
    },
  };
}

function startClient(context: vscode.ExtensionContext, command: string): void {
  const serverOptions: ServerOptions = {
    command,
    args: ["start"],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "shellscript" }],
    initializationOptions: buildInitializationOptions(),
  };

  client = new LanguageClient("bashls", "bashls", serverOptions, clientOptions);
  context.subscriptions.push(client);
  void client.start();
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const command = vscode.workspace
    .getConfiguration("bashls")
    .get<string>("path", "bashls");

  if (await isBashlsAvailable(command)) {
    startClient(context, command);
    return;
  }

  const cachedPath = context.globalState.get<string>(INSTALLED_PATH_KEY);
  if (cachedPath && fs.existsSync(cachedPath)) {
    startClient(context, cachedPath);
    return;
  }

  const install = "Install bashls";
  const openDocs = "Open install docs";
  const choice = await vscode.window.showErrorMessage(
    `bashls binary not found ('${command}'). Install it automatically, install it manually and ensure it's on your PATH, or set the "bashls.path" setting.`,
    install,
    openDocs,
  );

  if (choice === openDocs) {
    void vscode.env.openExternal(vscode.Uri.parse(INSTALL_DOCS_URL));
    return;
  }

  if (choice === install) {
    try {
      const binaryPath = await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: "Installing bashls",
        },
        (progress) => installBashls(context, progress),
      );
      await context.globalState.update(INSTALLED_PATH_KEY, binaryPath);
      startClient(context, binaryPath);
    } catch (error) {
      void vscode.window.showErrorMessage(
        `Failed to install bashls: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
