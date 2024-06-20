// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
import React from "react";
import { MDXProvider } from "@mdx-js/react";
import { VersionProvider } from "@site/src/components/Versions/versionContext";
import MDXComponents from "@theme/MDXComponents";
import Tabs from "@theme/Tabs";
import TabItem from "@theme/TabItem";
import { Card, Cards } from "@site/src/components/Cards";
import { Versioned, Version } from "@site/src/components/Versions";
export default function MDXContent({ children }) {
  const suiComponents = {
    ...MDXComponents,
    Card,
    Cards,
    Tabs,
    TabItem,
    Versioned,
    Version,
  };
  return (
    <VersionProvider>
      <MDXProvider components={suiComponents}>{children}</MDXProvider>
    </VersionProvider>
  );
}
