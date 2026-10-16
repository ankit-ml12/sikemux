import { StyleSheet, Text, View } from 'react-native';

import { sayFarewell, type Farewell as Reason } from '@/account/farewell';
import { Button } from '@/ui/controls';
import { Screen, useBottomGap } from '@/ui/screen';
import { type Palette, typeFor, useStyles } from '@/ui/theme';

const RESTORE = 'Sign in again within 30 days to restore it.';

const WORDS: Record<Reason, { title: string; body?: string }> = {
  removed: { title: 'This phone was removed from your account' },
  deleted: { title: 'Your account was deleted', body: RESTORE },
  'deleted-here': { title: 'Your account is deleted', body: RESTORE },
};

/** Says why the phone signed out, before the welcome screen. */
export function Farewell({ reason }: { reason: Reason }) {
  const styles = useStyles(makeStyles);
  const bottom = useBottomGap();
  const words = WORDS[reason];
  return (
    <Screen>
      <View style={styles.block}>
        <Text style={styles.title}>{words.title}</Text>
        {words.body ? <Text style={styles.body}>{words.body}</Text> : null}
      </View>
      <View style={[styles.footer, { paddingBottom: bottom }]}>
        <Button kind="primary" title="Continue" onPress={() => sayFarewell(null)} />
      </View>
    </Screen>
  );
}

const makeStyles = (colors: Palette) => {
  const type = typeFor(colors);
  return StyleSheet.create({
    block: { flex: 1, justifyContent: 'center', paddingHorizontal: 32, paddingBottom: 120 },
    title: { ...type.title, fontSize: 22, textAlign: 'center' },
    body: { ...type.body, textAlign: 'center', marginTop: 10 },
    footer: { paddingHorizontal: 16, paddingTop: 12 },
  });
};
