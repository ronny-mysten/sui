// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
const injectCode = async () => {
  return {
    name: "inject-code",
    //async loadContent() {
      //return "PEANUTBUTTER";
    //},
    async contentLoaded({ content, actions }) {
      console.log(content);
    },
    /* other lifecycle API */
  };
};

module.exports = injectCode;
