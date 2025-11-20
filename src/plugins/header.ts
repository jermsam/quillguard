import Header, { type HeaderData, type HeaderConfig } from "@editorjs/header";
import { API } from "@editorjs/editorjs";

/**
 * @description Constructor arguments for Header
 */
interface ConstructorArgs {
  /** Previously saved data */
  data: HeaderData | object;
  /** User config for the tool */
  config?: HeaderConfig;
  /** Editor.js API */
  api: API;
  /** Read-only mode flag */
  readOnly: boolean;
}
export class GHeader extends Header {
  constructor({ data, config, api, readOnly }: ConstructorArgs) {
    super({ data, config: config ?? {}, api, readOnly });
  }
}
