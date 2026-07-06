/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `enable` command */
  export type Enable = ExtensionPreferences & {}
  /** Preferences accessible in the `disable` command */
  export type Disable = ExtensionPreferences & {}
  /** Preferences accessible in the `status` command */
  export type Status = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `enable` command */
  export type Enable = {}
  /** Arguments passed to the `disable` command */
  export type Disable = {}
  /** Arguments passed to the `status` command */
  export type Status = {}
}

