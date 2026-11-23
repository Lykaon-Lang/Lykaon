// Lykaon VS Code extension.
// A thin LSP client for `lykaonc-cli lsp` plus run/test commands.
// Plain JavaScript, no build step.

const vscode = require('vscode');
const { spawn } = require('child_process');
const fs = require('fs');
const pathMod = require('path');

let proc = null;
let buffer = Buffer.alloc(0);
let nextId = 1;
const pending = new Map();
let diagnostics = null;
let output = null;

function cfg() {
    return vscode.workspace.getConfiguration('lykaon');
}

// Where the installer puts the tools.
const INSTALL_DIR = process.env.LOCALAPPDATA
    ? pathMod.join(process.env.LOCALAPPDATA, 'Lykaon')
    : null;

// The settings ship with a bare-name default, so "still the default" has to
// count as "not configured".
const BARE_DEFAULTS = {
    serverPath: 'lykaonc-cli',
    compilerPath: 'lykaonc-cli',
    interpreterPath: 'lykaon-cli'
};

// Find a Lykaon tool without relying on PATH. Windows keeps a stale PATH in
// processes that were already running when install.bat ran, so an open VS Code
// window would never see the new folder. Order:
//   1. the explicit setting, if it is not the untouched default
//   2. %LOCALAPPDATA%\Lykaon           (where install.bat copies the tools)
//   3. the folder of the open .lykaon file (portable: unzip and run)
//   4. the workspace folder
//   5. the bare name, which makes the OS search PATH (works after a restart)
function resolveTool(settingName, bareName) {
    const configured = (cfg().get(settingName) || '').trim();
    if (configured && configured !== BARE_DEFAULTS[settingName]) return configured;

    const exe = process.platform === 'win32' ? `${bareName}.exe` : bareName;
    const dirs = [];
    if (INSTALL_DIR) dirs.push(INSTALL_DIR);
    const editor = vscode.window.activeTextEditor;
    if (editor && editor.document.uri.scheme === 'file') {
        dirs.push(pathMod.dirname(editor.document.fileName));
    }
    for (const folder of vscode.workspace.workspaceFolders || []) {
        if (folder.uri.scheme === 'file') dirs.push(folder.uri.fsPath);
    }
    for (const dir of dirs) {
        const candidate = pathMod.join(dir, exe);
        try {
            if (fs.existsSync(candidate)) return candidate;
        } catch (_) { /* unreadable folder, keep looking */ }
    }
    return exe;
}

function send(msg) {
    if (!proc || !proc.stdin) return;
    const body = Buffer.from(JSON.stringify(msg), 'utf8');
    proc.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    proc.stdin.write(body);
}

function request(method, params) {
    return new Promise((resolve, reject) => {
        const id = nextId++;
        const timer = setTimeout(() => {
            pending.delete(id);
            reject(new Error(`LSP request timed out: ${method}`));
        }, 15000);
        pending.set(id, { resolve, reject, timer });
        send({ jsonrpc: '2.0', id, method, params });
    });
}

function notify(method, params) {
    send({ jsonrpc: '2.0', method, params });
}

function handle(msg) {
    if (msg.id !== undefined && pending.has(msg.id)) {
        const p = pending.get(msg.id);
        pending.delete(msg.id);
        clearTimeout(p.timer);
        if (msg.error) p.reject(new Error(msg.error.message || 'LSP error'));
        else p.resolve(msg.result);
        return;
    }
    if (msg.method === 'textDocument/publishDiagnostics' && diagnostics) {
        const uri = vscode.Uri.parse(msg.params.uri);
        const items = (msg.params.diagnostics || []).map(d => {
            const range = new vscode.Range(
                d.range.start.line, d.range.start.character,
                d.range.end.line, d.range.end.character
            );
            const diag = new vscode.Diagnostic(range, d.message, vscode.DiagnosticSeverity.Error);
            diag.source = 'lykaon';
            diag.code = d.code || '';
            return diag;
        });
        diagnostics.set(uri, items);
    }
}

function onData(chunk) {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
        const headerEnd = buffer.indexOf('\r\n\r\n');
        if (headerEnd < 0) return;
        const header = buffer.slice(0, headerEnd).toString('utf8');
        const m = header.match(/Content-Length:\s*(\d+)/i);
        if (!m) { buffer = buffer.slice(headerEnd + 4); continue; }
        const len = parseInt(m[1], 10);
        const start = headerEnd + 4;
        if (buffer.length < start + len) return;
        const body = buffer.slice(start, start + len).toString('utf8');
        buffer = buffer.slice(start + len);
        try { handle(JSON.parse(body)); } catch (_) { /* ignore bad frame */ }
    }
}

function startServer() {
    const cli = resolveTool('serverPath', 'lykaonc-cli');
    try {
        proc = spawn(cli, ['lsp'], { stdio: ['pipe', 'pipe', 'pipe'] });
    } catch (e) {
        vscode.window.showErrorMessage(`Lykaon: cannot start ${cli}: ${e}`);
        return;
    }
    proc.stdout.on('data', onData);
    proc.stderr.on('data', d => {
        if (output) output.appendLine(d.toString());
    });
    proc.on('error', e => {
        if (output) output.appendLine(`Lykaon: cannot start ${cli}: ${e}`);
        proc = null;
    });
    proc.on('exit', code => {
        if (output) output.appendLine(`Lykaon language server exited (${code})`);
        proc = null;
    });
    request('initialize', {
        processId: process.pid,
        rootUri: vscode.workspace.workspaceFolders?.[0]?.uri.toString() || null,
        capabilities: {}
    }).catch(err => {
        if (output) output.appendLine(`Lykaon LSP initialize failed: ${err}`);
    });
}

function documentUri(document) {
    return document.uri.toString();
}

function positionParams(document, position) {
    return {
        textDocument: { uri: documentUri(document) },
        position: { line: position.line, character: position.character }
    };
}

async function formattingEdits(document) {
    const result = await request('textDocument/formatting', {
        textDocument: { uri: documentUri(document) },
        options: { tabSize: 4, insertSpaces: true }
    });
    return (result || []).map(e => new vscode.TextEdit(
        new vscode.Range(e.range.start.line, e.range.start.character, e.range.end.line, e.range.end.character),
        e.newText
    ));
}

function activate(context) {
    diagnostics = vscode.languages.createDiagnosticCollection('lykaon');
    output = vscode.window.createOutputChannel('Lykaon');
    startServer();

    // M5d: offer the Lykaon file icons once. VS Code requires an icon theme to
    // be selected; this sets it on the first Yes (the user can change it later).
    if (vscode.workspace.getConfiguration().get('workbench.iconTheme') !== 'lykaon'
        && !context.globalState.get('lykaon.iconPrompted')) {
        context.globalState.update('lykaon.iconPrompted', true);
        vscode.window.showInformationMessage('Switch your active File Icon Theme to Lykaon to display .lykaon file icons?', 'Switch Theme', 'Keep Current')
            .then(choice => {
                if (choice === 'Switch Theme') {
                    vscode.workspace.getConfiguration().update('workbench.iconTheme', 'lykaon', vscode.ConfigurationTarget.Global);
                }
            });
    }

    // M5d: DAP debugging. VS Code launches `lykaonc-cli debug <file>` and
    // talks DAP to it over stdio (breakpoints, step, variables).
    context.subscriptions.push(
        vscode.debug.registerDebugAdapterDescriptorFactory('lykaon', {
            createDebugAdapterDescriptor(session) {
                const program = session.configuration.program
                    || vscode.window.activeTextEditor?.document.fileName;
                const cli = resolveTool('compilerPath', 'lykaonc-cli');
                return new vscode.DebugAdapterExecutable(cli, ['debug', program]);
            }
        })
    );

    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument(doc => {
            if (doc.languageId === 'lykaon') notify('textDocument/didOpen', {
                textDocument: { uri: documentUri(doc), languageId: 'lykaon', version: doc.version, text: doc.getText() }
            });
        }),
        vscode.workspace.onDidChangeTextDocument(e => {
            if (e.document.languageId === 'lykaon') notify('textDocument/didChange', {
                textDocument: { uri: documentUri(e.document), version: e.document.version },
                contentChanges: [{ text: e.document.getText() }]
            });
        }),
        vscode.workspace.onDidCloseTextDocument(doc => {
            if (doc.languageId === 'lykaon') notify('textDocument/didClose', {
                textDocument: { uri: documentUri(doc) }
            });
        }),
        vscode.workspace.onWillSaveTextDocument(e => {
            if (e.document.languageId !== 'lykaon' || !cfg().get('formatOnSave')) return;
            e.waitUntil(formattingEdits(e.document).then(edits => edits));
        }),
        vscode.languages.registerDocumentFormattingEditProvider('lykaon', {
            provideDocumentFormattingEdits: formattingEdits
        }),
        vscode.languages.registerCompletionItemProvider('lykaon', {
            async provideCompletionItems(document, position) {
                const result = await request('textDocument/completion', positionParams(document, position));
                return (result || []).map(item => {
                    const ci = new vscode.CompletionItem(item.label, vscode.CompletionItemKind.Keyword);
                    ci.detail = 'Lykaon';
                    return ci;
                });
            }
        }),
        vscode.languages.registerHoverProvider('lykaon', {
            async provideHover(document, position) {
                const result = await request('textDocument/hover', positionParams(document, position));
                if (!result || !result.contents) return null;
                return new vscode.Hover(new vscode.MarkdownString(result.contents.value));
            }
        }),
        vscode.languages.registerDefinitionProvider('lykaon', {
            async provideDefinition(document, position) {
                const result = await request('textDocument/definition', positionParams(document, position));
                return (result || []).map(loc => new vscode.Location(
                    vscode.Uri.parse(loc.uri),
                    new vscode.Range(loc.range.start.line, loc.range.start.character, loc.range.end.line, loc.range.end.character)
                ));
            }
        }),
        vscode.languages.registerDocumentSymbolProvider('lykaon', {
            provideDocumentSymbols(document) {
                return request('textDocument/documentSymbol', { textDocument: { uri: documentUri(document) } })
                    .then(syms => (syms || []).map(s => {
                        let loc = new vscode.Location(document.uri, new vscode.Range(0, 0, 0, 0));
                        if (s.location && s.location.range) {
                            loc = new vscode.Location(document.uri, new vscode.Range(
                                s.location.range.start.line, s.location.range.start.character,
                                s.location.range.end.line, s.location.range.end.character));
                        }
                        return new vscode.SymbolInformation(s.name, s.kind, s.name, loc);
                    }));
            }
        }),
        vscode.languages.registerReferenceProvider('lykaon', {
            provideReferences(document, position) {
                return request('textDocument/references', {
                    textDocument: { uri: documentUri(document) },
                    position: { line: position.line, character: position.character },
                    context: { includeDeclaration: true }
                }).then(locs => (locs || []).map(l => new vscode.Location(
                    vscode.Uri.parse(l.uri || documentUri(document)),
                    new vscode.Range(l.range.start.line, l.range.start.character,
                                     l.range.end.line, l.range.end.character))));
            }
        }),
        vscode.languages.registerRenameProvider('lykaon', {
            provideRenameEdits(document, position, newName) {
                return request('textDocument/rename', {
                    textDocument: { uri: documentUri(document) },
                    position: { line: position.line, character: position.character },
                    newName
                }).then(res => {
                    const edit = new vscode.WorkspaceEdit();
                    const changes = (res && res.changes) || {};
                    for (const uri in changes) {
                        for (const e of changes[uri]) {
                            edit.replace(vscode.Uri.parse(uri),
                                new vscode.Range(e.range.start.line, e.range.start.character,
                                                 e.range.end.line, e.range.end.character),
                                e.newText);
                        }
                    }
                    return edit;
                });
            }
        }),
        vscode.commands.registerCommand('lykaon.run', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.languageId !== 'lykaon') {
                vscode.window.showInformationMessage('Lykaon: open a .lykaon file first.');
                return;
            }
            const terminal = vscode.window.createTerminal({
                name: 'Lykaon: Run',
                shellPath: resolveTool('interpreterPath', 'lykaon-cli'),
                shellArgs: ['run', editor.document.fileName]
            });
            terminal.show();
        }),
        vscode.commands.registerCommand('lykaon.test', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.languageId !== 'lykaon') {
                vscode.window.showInformationMessage('Lykaon: open a .lykaon file first.');
                return;
            }
            const terminal = vscode.window.createTerminal({
                name: 'Lykaon: Test',
                shellPath: resolveTool('compilerPath', 'lykaonc-cli'),
                shellArgs: ['test', editor.document.fileName]
            });
            terminal.show();
        })
    );
}

function deactivate() {
    if (proc) {
        try { proc.kill(); } catch (_) {}
        proc = null;
    }
    if (diagnostics) diagnostics.dispose();
}

module.exports = { activate, deactivate, resolveTool };