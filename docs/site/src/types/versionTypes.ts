// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

export type VersionedProps = {
  children: React.ReactNode;
};

export interface VersionProps {
  version: string;
  featureID: string;
  curNetwork: NetworkProfile;
  versionsIncluded: string[];
}

export type Version = {
  network: string;
  display: string;
  version: string;
};

export type PluginData = {
  versions: Version[];
};

export type NetworkProfile = {
  network: string;
  display: string;
  version: string;
  index: number;
};

export type VersionButtonProps = {
  selNetwork: (display: string, version: string) => void;
  versions: Version[];
};
