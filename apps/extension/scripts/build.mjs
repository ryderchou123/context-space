import { cp, mkdir } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');const dist=resolve(root,'dist');await mkdir(resolve(dist,'icons'),{recursive:true});for(const file of ['manifest.json','popup.html','popup.css'])await cp(resolve(root,'public',file),resolve(dist,file));const source=resolve(root,'../desktop/src-tauri/icons');await cp(resolve(source,'32x32.png'),resolve(dist,'icons/16.png'));await cp(resolve(source,'32x32.png'),resolve(dist,'icons/32.png'));await cp(resolve(source,'128x128.png'),resolve(dist,'icons/128.png'));
