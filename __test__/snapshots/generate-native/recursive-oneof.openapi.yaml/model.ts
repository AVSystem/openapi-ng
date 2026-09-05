export type TreeNode = {
  leaf: string;
} | {
  children: TreeNode[];
};
