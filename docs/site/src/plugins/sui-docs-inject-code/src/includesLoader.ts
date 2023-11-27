/**
 * Copyright (c) Bucher + Suter.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import { IncludesLoaderOptions } from './types';
import fs from 'fs';
import path from 'path';

// TODO temporary until Webpack5 export this type
// see https://github.com/webpack/webpack/issues/11630
interface Loader extends Function {
  (this: any, source: string): string | Buffer | void | undefined;
}

const markdownLoader: Loader = function (source) {

  let fileString = source as string;
  const callback = this.async();

  const options = this.getOptions() as unknown as IncludesLoaderOptions;
  const markdownFilename = path.basename(this.resourcePath);
  const markdownFilepath = path.dirname(this.resourcePath);
  const repoPath = path.join(__dirname, '../../../../../..');


  // Do not load and render markdown files without docusaurus header.
  // These files are only used to be included in other files and should not generate their own web page
  if (fileString.length >= 3 && fileString.substring(0,3) !== '---') {
    return (callback && callback(null, ""));
  }

  function addMarkdownIncludes(fileContent: string) {
    let res = fileContent;
    const matches = fileContent.match(/\{@\w+: .+\}/g);
    if (matches) {
      matches.forEach(match => {
        const replacer = new RegExp(match, 'g');
        if (match.startsWith('{@inject: ')) {
          const injectFileFull = match.substring(10, match.length - 1);
          const injectFile = injectFileFull.substring(0, injectFileFull.indexOf('#') > 0 ? injectFileFull.indexOf('#') : injectFileFull.length);
          let fileExt = injectFile.substring(injectFile.lastIndexOf('.') + 1);
          let language = "";
          const fullPath = path.join(repoPath, injectFile);

          switch(fileExt){
            case "move":
              language = "rust";
              break;
            case "toml":
              language = "rust";
              break;
            case "lock":
              language = "rust";
              break;
            case "sh":
              language = "shell";
              break;
            case 'mdx':
              language = "markdown";
              break;
            case 'tsx':
              language = "ts";
              break;
            default:
              language = fileExt;
          }
          
          if (fs.existsSync(fullPath)) {
            let injectFileContent = fs.readFileSync(fullPath, "utf8");
            const marker = injectFileFull.indexOf('#') > 0 ? injectFileFull.substring(injectFileFull.indexOf('#')) : null;
            if (marker) {
              const regexStr = `\/\/\\s*?docs::${marker}([\\s\\S]*?)\/\/\\s*?docs::\/\\s*?${marker}`;
              const closingsStr = `\/\/\\s*?docs::\/${marker}([)};]+)`;
              const closingRE = new RegExp(closingsStr);
              const regex = new RegExp(regexStr, "g");
              const match = regex.exec(injectFileContent);
              const closingStr = closingRE.exec(injectFileContent);
              if (match){
                injectFileContent = match[1];
              }
              if (closingStr){
                const closingTotal = closingStr[1].length;
                let closingArray = [];
                for (let i = 0; i < closingTotal; i++) {
                  const currentChar = closingStr[1][i];
                  const nextChar = closingStr[1][i + 1];
                
                  if (nextChar === ';') {
                    closingArray.push(currentChar + nextChar);
                    i++; 
                  } else {
                    closingArray.push(currentChar);
                  }
                }
                const totClosings = closingArray.length;
                //let closing = `${'\t'.repeat(totClosings - 1)}${closingArray[0]}`;
                let closing = "";
                for (let j = 0; j < totClosings; j++){
                  let space = '  '.repeat((totClosings-1)-j)
                  closing += `\n${space}${closingArray[j]}`
                }
                injectFileContent = injectFileContent.trim() + closing;
              } else {
                console.log("NOT")
              }
            }

            injectFileContent = injectFileContent.replace(/\/\/\s*docs\/?::.*/g, '');

            injectFileContent = `\`\`\`${language} title=${injectFile}\n${injectFileContent}\n\`\`\``;
            
            res = res.replace(replacer, injectFileContent);
            res = addMarkdownIncludes(res);
          }

          else {
            res = res.replace(replacer, `\n> code to inject not found: ${injectFile} --> ${fullPath}\n`);
          }
        }
        else {
          const parts = match.substring(2, match.length - 3).split(': ');
          if (parts.length === 2) {
            if (options.embeds) {
              for (const embed of options.embeds) {
                if (embed.key === parts[0]) {
                  const embedResult = embed.embedFunction(parts[1]);
                  res = res.replace(replacer, embedResult);
                }
              }
            }
          }
        }
      });
    }
    return res;
  }

  function replacePlaceHolders(documentContent: string) {
    var res = documentContent;
    if (options.replacements) {
      var placeHolders = [...options.replacements];
      if (!placeHolders) {
        placeHolders = [];
      }
      placeHolders.push({ key: '{ContainerMarkdown}', value: markdownFilename });
      placeHolders.forEach(replacement => {
        const replacer = new RegExp(replacement.key, 'g');
        res = res.replace(replacer, replacement.value);
      });
    }
    return res;
  }

  fileString = replacePlaceHolders(addMarkdownIncludes(fileString));

  return (callback && callback(null, fileString));
};

export default markdownLoader;
