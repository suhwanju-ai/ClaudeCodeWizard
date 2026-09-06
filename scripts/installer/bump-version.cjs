#!/usr/bin/env node
// build-installer.bat/.sh이 설치 파일을 만들기 직전에 호출한다.
// src-tauri/tauri.conf.json의 "version"을 유일한 출처로 삼아 patch를 1 올리고,
// 같은 값을 package.json / src-tauri/Cargo.toml에도 반영해 세 파일이 어긋나지 않게 한다.

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..", "..");
const tauriConfPath = path.join(root, "src-tauri", "tauri.conf.json");
const packageJsonPath = path.join(root, "package.json");
const cargoTomlPath = path.join(root, "src-tauri", "Cargo.toml");

const TAURI_VERSION_RE = /"version":\s*"(\d+)\.(\d+)\.(\d+)"/;
const tauriText = fs.readFileSync(tauriConfPath, "utf8");
const m = TAURI_VERSION_RE.exec(tauriText);
if (!m) throw new Error(`${tauriConfPath}에서 version을 찾지 못했습니다.`);
const [, major, minor, patch] = m;
const current = `${major}.${minor}.${patch}`;
const next = `${major}.${minor}.${Number(patch) + 1}`;

function bumpJsonFile(file) {
  const text = fs.readFileSync(file, "utf8");
  const re = /"version":\s*"\d+\.\d+\.\d+"/;
  if (!re.test(text)) throw new Error(`${file}에서 "version" 줄을 찾지 못했습니다.`);
  fs.writeFileSync(file, text.replace(re, `"version": "${next}"`), "utf8");
}

function bumpCargoToml(file) {
  const lines = fs.readFileSync(file, "utf8").split("\n");
  let inPackageSection = false;
  let replaced = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (/^\[.*\]\s*$/.test(line)) {
      inPackageSection = line.trim() === "[package]";
      continue;
    }
    // 🔒 [package] 섹션 안의 version만 바꾼다 — [dependencies]에도 같은 이름의
    //    "version = ..." 줄이 있어(예: tauri = { version = "2", ... }), 섹션 밖에서
    //    매칭하면 의존성 버전을 잘못 덮어쓴다.
    if (inPackageSection && /^version\s*=\s*"\d+\.\d+\.\d+"/.test(line)) {
      lines[i] = `version = "${next}"`;
      replaced = true;
      inPackageSection = false; // [package]의 version은 한 번만 등장한다
    }
  }
  if (!replaced) throw new Error(`${file}의 [package] 섹션에서 version 줄을 찾지 못했습니다.`);
  fs.writeFileSync(file, lines.join("\n"), "utf8");
}

bumpJsonFile(tauriConfPath);
bumpJsonFile(packageJsonPath);
bumpCargoToml(cargoTomlPath);

console.log(`[bump-version] ${current} -> ${next}  (tauri.conf.json / package.json / Cargo.toml)`);
