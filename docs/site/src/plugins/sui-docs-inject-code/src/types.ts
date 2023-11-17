/**
 * Copyright (c) Bucher + Suter.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

export type IncludesLoaderOptions = {
    replacements?: IncludeLoaderOptionReplacements,
    embeds?: IncludeLoaderOptionEmbeds,    
    sharedFolders: SharedFoldersOption
};

type HtmlTags = string | HtmlTagObject | (string | HtmlTagObject)[];

interface HtmlTagObject {
  /**
   * Attributes of the HTML tag
   * E.g. `{'disabled': true, 'value': 'demo', 'rel': 'preconnect'}`
   */
  attributes?: {
    [attributeName: string]: string | boolean;
  };
  /**
   * The tag name e.g. `div`, `script`, `link`, `meta`
   */
  tagName: string;
  /**
   * The inner HTML
   */
  innerHTML?: string;
}

export type IncludeLoaderOptionReplacements = {key: string, value: string}[];

export type IncludeLoaderOptionEmbeds = {key: string, embedFunction(code:string): string }[];

export type SharedFoldersOption = undefined | {source: string, target: string}[];

export type InjectHtmlTagsOption = undefined | { headTags?: HtmlTags; preBodyTags?: HtmlTags; postBodyTags?: HtmlTags;}[];

export type IncludesPluginOptions = {
    replacements?: IncludeLoaderOptionReplacements,
    sharedFolders?: SharedFoldersOption,
    postBuildDeletedFolders?: string[],
    embeds?: IncludeLoaderOptionEmbeds,
    injectedHtmlTags?: InjectHtmlTagsOption
}

export type VersionInfo = {
  version: string,
  urlAddIn: string,
}
