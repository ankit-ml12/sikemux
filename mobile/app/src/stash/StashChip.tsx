import { Pressable, StyleSheet, Text, View } from 'react-native';

import { Icon } from '@/ui/Icon';
import { fonts, type Palette, useColors, useStyles } from '@/ui/theme';
import type { StashItem } from './api';
import { Face, useBearer } from './StashPicker';
import { useStash } from './store';
import { itemTitle } from './words';

/** A stash item as it sits in the composer: its picture or glyph and its title. */
export function StashPreview({ item }: { item: StashItem }) {
  const styles = useStyles(makeStyles);
  const bearer = useBearer();
  return (
    <View style={styles.preview}>
      <Face item={item} bearer={bearer} size={30} />
      <Text style={styles.title} numberOfLines={1}>
        {itemTitle(item)}
      </Text>
    </View>
  );
}

/** The stash item a file came from, while the stash still holds it. */
export function useStashItem(id: string | undefined): StashItem | undefined {
  const items = useStash().items;
  return id === undefined ? undefined : items.find((item) => item.id === id);
}

/** A stashed link or text picked for the next message, with its × to take it off. */
export function StashChip({ item, onRemove }: { item: StashItem; onRemove: () => void }) {
  const colors = useColors();
  const styles = useStyles(makeStyles);
  return (
    <View style={styles.chip}>
      <StashPreview item={item} />
      <Pressable
        onPress={onRemove}
        hitSlop={10}
        style={styles.remove}
        accessibilityRole="button"
        accessibilityLabel={`Remove ${itemTitle(item)}`}>
        <Icon name="IconClose" size={10} color={colors.ground} />
      </Pressable>
    </View>
  );
}

const makeStyles = (colors: Palette) =>
  StyleSheet.create({
    chip: {
      height: 44,
      maxWidth: 220,
      borderRadius: 9,
      borderWidth: 1,
      borderColor: colors.border,
      backgroundColor: colors.sunken,
      overflow: 'hidden',
      justifyContent: 'center',
    },
    preview: { flexDirection: 'row', alignItems: 'center', gap: 8, paddingLeft: 6, paddingRight: 28, maxWidth: 220 },
    title: { flexShrink: 1, fontFamily: fonts.uiMedium, fontSize: 12.5, color: colors.ink },
    remove: {
      position: 'absolute',
      top: 3,
      right: 3,
      width: 18,
      height: 18,
      borderRadius: 9,
      backgroundColor: colors.ink,
      alignItems: 'center',
      justifyContent: 'center',
    },
  });
