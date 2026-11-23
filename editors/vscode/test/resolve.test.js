// Unit test for the compiler/interpreter lookup in extension.js.
// Run with: node test/resolve.test.js
//
// The extension must work in a VS Code window that was already open when
// install.bat ran, because Windows keeps the old PATH in running processes.
// So it has to find the tools through %LOCALAPPDATA%\Lykaon, not PATH.

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const Module = require('module');

// --- fake VS Code API -------------------------------------------------------
let settings = {};
let activeFile = null;
let folders = [];

const vscodeStub = {
    workspace: {
        getConfiguration: () => ({
            get: name => settings[name],
        }),
        get workspaceFolders() {
            return folders;
        },
    },
    window: {
        get activeTextEditor() {
            return activeFile
                ? { document: { uri: { scheme: 'file' }, fileName: activeFile } }
                : undefined;
        },
    },
};

const originalLoad = Module._load;
Module._load = function (request, parent, isMain) {
    if (request === 'vscode') return vscodeStub;
    return originalLoad.call(this, request, parent, isMain);
};

// --- fixture ----------------------------------------------------------------
const localAppData = fs.mkdtempSync(path.join(os.tmpdir(), 'lykaon-localappdata-'));
const installDir = path.join(localAppData, 'Lykaon');
fs.mkdirSync(installDir);
const exeSuffix = process.platform === 'win32' ? '.exe' : '';
fs.writeFileSync(path.join(installDir, 'lykaonc-cli' + exeSuffix), '');
fs.writeFileSync(path.join(installDir, 'lykaon-cli' + exeSuffix), '');

process.env.LOCALAPPDATA = localAppData;

const ext = require('../extension.js');

// --- tests ------------------------------------------------------------------
// 1. Nothing configured and an open VS Code window: still finds the installed copy.
settings = { serverPath: 'lykaonc-cli', compilerPath: 'lykaonc-cli', interpreterPath: 'lykaon-cli' };
assert.strictEqual(
    ext.resolveTool('compilerPath', 'lykaonc-cli'),
    path.join(installDir, 'lykaonc-cli' + exeSuffix),
    'should fall back to the installed folder'
);
assert.strictEqual(
    ext.resolveTool('interpreterPath', 'lykaon-cli'),
    path.join(installDir, 'lykaon-cli' + exeSuffix),
    'should fall back to the installed folder for the interpreter'
);

// 2. An explicit setting wins.
const custom = path.join(os.tmpdir(), 'custom-lykaonc' + exeSuffix);
settings = { compilerPath: custom };
assert.strictEqual(ext.resolveTool('compilerPath', 'lykaonc-cli'), custom);

// 3. Portable use: the folder of the open .lykaon file, when the install folder
//    has no tools (an unzipped copy that was never installed).
process.env.LOCALAPPDATA = path.join(os.tmpdir(), 'lykaon-does-not-exist');
const portable = fs.mkdtempSync(path.join(os.tmpdir(), 'lykaon-portable-'));
fs.writeFileSync(path.join(portable, 'lykaonc-cli' + exeSuffix), '');
delete require.cache[require.resolve('../extension.js')];
const extPortable = require('../extension.js');
settings = { compilerPath: 'lykaonc-cli' };
activeFile = path.join(portable, 'Hello.lykaon');
folders = [];
assert.strictEqual(
    extPortable.resolveTool('compilerPath', 'lykaonc-cli'),
    path.join(portable, 'lykaonc-cli' + exeSuffix),
    'should look next to the open file'
);

// 4. Nothing anywhere: fall back to the bare name so PATH can resolve it.
activeFile = null;
assert.strictEqual(extPortable.resolveTool('compilerPath', 'lykaonc-cli'), 'lykaonc-cli' + exeSuffix);

// 5. Workspace folder is used when there is no open file.
folders = [{ uri: { scheme: 'file', fsPath: portable } }];
assert.strictEqual(
    extPortable.resolveTool('compilerPath', 'lykaonc-cli'),
    path.join(portable, 'lykaonc-cli' + exeSuffix),
    'should look in the workspace folder'
);

Module._load = originalLoad;
console.log('resolve.test.js: 5 checks passed');
